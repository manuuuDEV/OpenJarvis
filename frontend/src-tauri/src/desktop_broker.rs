//! Native, constrained Windows UI Automation broker.
//!
//! This module deliberately does **not** expose SendInput, shell execution,
//! coordinate clicks, clipboard access, elevation, or a generic automation
//! language. The cloud-backed agent can only submit a reviewed structured plan
//! through the Python approval queue. The broker revalidates target and steps
//! immediately before each native interaction.

use std::time::Duration;

use getrandom::fill;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use windows::{
    core::{BSTR, PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, HWND},
        System::{
            Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED},
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
            Variant::VARIANT,
        },
        UI::{
            Accessibility::{
                CUIAutomation, IUIAutomation, IUIAutomationElement,
                IUIAutomationInvokePattern, IUIAutomationValuePattern,
                TreeScope_Subtree, UIA_AutomationIdPropertyId, UIA_ButtonControlTypeId,
                UIA_EditControlTypeId, UIA_HyperlinkControlTypeId, UIA_InvokePatternId,
                UIA_ListItemControlTypeId, UIA_MenuItemControlTypeId, UIA_NamePropertyId,
                UIA_TabItemControlTypeId, UIA_TextControlTypeId, UIA_ValuePatternId,
                UIA_CONTROLTYPE_ID,
            },
            WindowsAndMessaging::{FindWindowW, GetWindowThreadProcessId, SetForegroundWindow},
        },
    },
};

const BROKER_HEADER: &str = "x-openjarvis-desktop-broker";
const MAX_STEPS: usize = 12;
const MAX_TEXT_CHARS: usize = 4_000;
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const SENSITIVE_TERMS: &[&str] = &[
    "account", "bank", "banking", "bonifico", "card", "checkout", "credential",
    "credit card", "cvv", "iban", "login", "otp", "password", "payment", "pin",
    "purchase", "recovery", "sign in", "transfer", "two factor", "wallet",
];

#[derive(Debug, Deserialize)]
struct ApprovedPlanIds {
    ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ClaimedPlan {
    id: String,
    plan: DesktopPlan,
}

#[derive(Debug, Deserialize)]
struct DesktopPlan {
    version: u8,
    summary: String,
    target: DesktopTarget,
    steps: Vec<DesktopStep>,
}

#[derive(Debug, Deserialize)]
struct DesktopTarget {
    application: String,
    window_title: String,
}

#[derive(Debug, Deserialize)]
struct DesktopStep {
    #[serde(rename = "type")]
    kind: String,
    element: Option<DesktopElement>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DesktopElement {
    name: Option<String>,
    automation_id: Option<String>,
    control_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct DesktopPlanCompletion<'a> {
    success: bool,
    summary: &'a str,
}

/// Create a fresh 256-bit capability secret for one backend/broker lifetime.
pub fn launch_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    fill(&mut bytes).map_err(|error| format!("Could not create broker secret: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Start the local polling worker after the Python server health check passes.
pub fn spawn_worker(token: String, api_base: String) {
    tauri::async_runtime::spawn(async move {
        let mut headers = HeaderMap::new();
        let value = match HeaderValue::from_str(&token) {
            Ok(value) => value,
            Err(_) => return,
        };
        headers.insert(BROKER_HEADER, value);
        let client = match reqwest::Client::builder().default_headers(headers).build() {
            Ok(client) => client,
            Err(_) => return,
        };
        broker_loop(client, api_base).await;
    });
}

async fn broker_loop(client: reqwest::Client, api_base: String) {
    loop {
        sleep(POLL_INTERVAL).await;
        let listed = client
            .get(format!("{api_base}/v1/approvals/desktop-plans/approved"))
            .send()
            .await;
        let response = match listed {
            Ok(response) if response.status().is_success() => response,
            Ok(response) if response.status().as_u16() == 403 => return,
            _ => continue,
        };
        let approved = match response.json::<ApprovedPlanIds>().await {
            Ok(approved) => approved,
            Err(_) => continue,
        };
        for action_id in approved.ids {
            let claimed = client
                .post(format!(
                    "{api_base}/v1/approvals/{action_id}/desktop-plan/claim"
                ))
                .send()
                .await;
            let response = match claimed {
                Ok(response) if response.status().is_success() => response,
                _ => continue,
            };
            let claimed = match response.json::<ClaimedPlan>().await {
                Ok(claimed) => claimed,
                Err(_) => continue,
            };
            let action_id = claimed.id;
            let outcome = tokio::task::spawn_blocking(move || execute_plan(claimed.plan))
                .await
                .ok()
                .and_then(Result::ok);
            let (success, summary) = match outcome {
                Some(summary) => (true, summary),
                None => (false, "Desktop plan did not complete.".to_string()),
            };
            let completion = DesktopPlanCompletion {
                success,
                summary: &summary,
            };
            let _ = client
                .post(format!(
                    "{api_base}/v1/approvals/{action_id}/desktop-plan/complete"
                ))
                .json(&completion)
                .send()
                .await;
        }
    }
}

fn execute_plan(plan: DesktopPlan) -> Result<String, String> {
    validate_plan(&plan)?;
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|error| format!("UI Automation initialization failed: {error}"))?;
        let result = execute_plan_with_com(&plan);
        CoUninitialize();
        result
    }
}

unsafe fn execute_plan_with_com(plan: &DesktopPlan) -> Result<String, String> {
    let title = wide_null(&plan.target.window_title);
    let hwnd = FindWindowW(None, PCWSTR(title.as_ptr()))
        .map_err(|_| "Could not locate the approved target window".to_string())?;
    if hwnd.0.is_null() {
        return Err("Approved target window is not open".to_string());
    }
    verify_target_process(hwnd, &plan.target.application)?;
    let _ = SetForegroundWindow(hwnd);

    let automation: IUIAutomation = CoCreateInstance(
        &CUIAutomation,
        None,
        CLSCTX_INPROC_SERVER,
    )
    .map_err(|error| format!("Could not create UI Automation client: {error}"))?;
    let root = automation
        .ElementFromHandle(hwnd)
        .map_err(|_| "Could not bind UI Automation to the approved window".to_string())?;
    let target_pid = root
        .CurrentProcessId()
        .map_err(|_| "Could not verify the approved window process".to_string())?;

    let mut observations = Vec::new();
    for step in &plan.steps {
        if let Some(observation) = execute_step(&automation, &root, target_pid, step)? {
            observations.push(observation);
        }
    }
    if observations.is_empty() {
        Ok("Desktop plan completed.".to_string())
    } else {
        Ok(observations.join("\n\n"))
    }
}

unsafe fn execute_step(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    target_pid: i32,
    step: &DesktopStep,
) -> Result<Option<String>, String> {
    match step.kind.as_str() {
        "focus_window" => {
            root.SetFocus()
                .map_err(|_| "Could not focus the approved window".to_string())?;
            Ok(None)
        }
        "inspect_window" => {
            let title = root
                .CurrentName()
                .map_err(|_| "Could not inspect the approved window".to_string())?
                .to_string();
            Ok(Some(truncate_observation(title)))
        }
        "read_accessible_text" => {
            let element = find_verified_element(automation, root, target_pid, step)?;
            let text = element
                .CurrentName()
                .map_err(|_| "Could not read the approved UI element".to_string())?
                .to_string();
            Ok(Some(truncate_observation(text)))
        }
        "invoke_element" => {
            let element = find_verified_element(automation, root, target_pid, step)?;
            let pattern: IUIAutomationInvokePattern = element
                .GetCurrentPatternAs(UIA_InvokePatternId)
                .map_err(|_| "Approved element does not support Invoke".to_string())?;
            pattern
                .Invoke()
                .map_err(|_| "Approved UI element could not be invoked".to_string())?;
            Ok(None)
        }
        "set_text" => {
            let text = step
                .text
                .as_deref()
                .ok_or_else(|| "Text step has no value".to_string())?;
            let element = find_verified_element(automation, root, target_pid, step)?;
            let pattern: IUIAutomationValuePattern = element
                .GetCurrentPatternAs(UIA_ValuePatternId)
                .map_err(|_| "Approved element does not support safe text input".to_string())?;
            if pattern
                .CurrentIsReadOnly()
                .map_err(|_| "Could not inspect approved text field".to_string())?
                .as_bool()
            {
                return Err("Approved text field is read-only".to_string());
            }
            let value = BSTR::from(text);
            pattern
                .SetValue(&value)
                .map_err(|_| "Approved text could not be entered".to_string())?;
            Ok(None)
        }
        _ => Err("Unsupported desktop step".to_string()),
    }
}

unsafe fn find_verified_element(
    automation: &IUIAutomation,
    root: &IUIAutomationElement,
    target_pid: i32,
    step: &DesktopStep,
) -> Result<IUIAutomationElement, String> {
    let selector = step
        .element
        .as_ref()
        .ok_or_else(|| "Desktop step has no verified element selector".to_string())?;
    let (property, value) = match (&selector.automation_id, &selector.name) {
        (Some(automation_id), _) => (UIA_AutomationIdPropertyId, automation_id),
        (None, Some(name)) => (UIA_NamePropertyId, name),
        _ => return Err("Element selector has no stable identity".to_string()),
    };
    let value = VARIANT::from(BSTR::from(value));
    let condition = automation
        .CreatePropertyCondition(property, &value)
        .map_err(|_| "Could not create UI element selector".to_string())?;
    let element = root
        .FindFirst(TreeScope_Subtree, &condition)
        .map_err(|_| "Approved UI element is not available in the target window".to_string())?;
    if element
        .CurrentProcessId()
        .map_err(|_| "Could not verify UI element process".to_string())?
        != target_pid
    {
        return Err("UI element belongs to a different process".to_string());
    }
    if let Some(expected_name) = &selector.name {
        let actual_name = element
            .CurrentName()
            .map_err(|_| "Could not verify UI element name".to_string())?
            .to_string();
        if &actual_name != expected_name {
            return Err("UI element name changed after plan approval".to_string());
        }
    }
    if let Some(expected_id) = &selector.automation_id {
        let actual_id = element
            .CurrentAutomationId()
            .map_err(|_| "Could not verify UI element automation ID".to_string())?
            .to_string();
        if &actual_id != expected_id {
            return Err("UI element identity changed after plan approval".to_string());
        }
    }
    if let Some(expected_control_type) = &selector.control_type {
        let expected = expected_control_type_id(expected_control_type)?;
        let actual = element
            .CurrentControlType()
            .map_err(|_| "Could not verify UI element control type".to_string())?;
        if actual != expected {
            return Err("UI element control type changed after plan approval".to_string());
        }
    }
    if !element
        .CurrentIsEnabled()
        .map_err(|_| "Could not inspect UI element state".to_string())?
        .as_bool()
    {
        return Err("Approved UI element is disabled".to_string());
    }
    if element
        .CurrentIsPassword()
        .map_err(|_| "Could not inspect UI element sensitivity".to_string())?
        .as_bool()
    {
        return Err("Password controls are never automated".to_string());
    }
    Ok(element)
}

unsafe fn verify_target_process(hwnd: HWND, expected_path: &str) -> Result<(), String> {
    let mut pid = 0_u32;
    if GetWindowThreadProcessId(hwnd, Some(&mut pid)) == 0 || pid == 0 {
        return Err("Could not identify the approved window process".to_string());
    }
    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
        .map_err(|_| "Could not inspect the approved window process".to_string())?;
    let mut buffer = vec![0_u16; 32_768];
    let mut length = buffer.len() as u32;
    let result = QueryFullProcessImageNameW(
        process,
        PROCESS_NAME_WIN32,
        PWSTR(buffer.as_mut_ptr()),
        &mut length,
    );
    let _ = CloseHandle(process);
    result.map_err(|_| "Could not read the approved process image path".to_string())?;
    let actual = String::from_utf16_lossy(&buffer[..length as usize]);
    if !actual.eq_ignore_ascii_case(expected_path) {
        return Err("The target window process does not match the approved executable".to_string());
    }
    Ok(())
}

fn validate_plan(plan: &DesktopPlan) -> Result<(), String> {
    if plan.version != 1 || plan.steps.is_empty() || plan.steps.len() > MAX_STEPS {
        return Err("Desktop plan has an invalid version or step count".to_string());
    }
    if !is_absolute_windows_exe(&plan.target.application)
        || contains_sensitive_term(&plan.target.application)
        || contains_sensitive_term(&plan.target.window_title)
        || contains_sensitive_term(&plan.summary)
    {
        return Err("Desktop plan target is not allowed".to_string());
    }
    for step in &plan.steps {
        if !matches!(
            step.kind.as_str(),
            "focus_window" | "inspect_window" | "read_accessible_text" | "invoke_element" | "set_text"
        ) {
            return Err("Desktop plan contains an unsupported action".to_string());
        }
        if let Some(element) = &step.element {
            for value in [&element.name, &element.automation_id, &element.control_type]
                .into_iter()
                .flatten()
            {
                if contains_sensitive_term(value) {
                    return Err("Desktop plan targets a sensitive UI element".to_string());
                }
            }
        } else if step.kind != "focus_window" && step.kind != "inspect_window" {
            return Err("Desktop plan action requires a UI element".to_string());
        }
        if step.kind == "set_text" {
            let text = step
                .text
                .as_deref()
                .ok_or_else(|| "Text action has no content".to_string())?;
            if text.is_empty() || text.len() > MAX_TEXT_CHARS || contains_sensitive_term(text) {
                return Err("Desktop plan text is not allowed".to_string());
            }
        }
    }
    Ok(())
}

fn truncate_observation(value: String) -> String {
    let limited: String = value.chars().take(MAX_TEXT_CHARS).collect();
    if contains_sensitive_term(&limited) || looks_sensitive_observation(&limited) {
        "[REDACTED: sensitive accessible text withheld]".to_string()
    } else {
        limited
    }
}

/// Conservative local pre-filter. Server-side redaction remains the final
/// boundary, but raw accessibility text should not leave the native broker
/// when it resembles a credential, contact identifier, or long number.
fn looks_sensitive_observation(value: &str) -> bool {
    let trimmed = value.trim();
    let lowered = trimmed.to_ascii_lowercase();
    if ["sk-", "gsk_", "hf_", "bearer "]
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
        || lowered.contains("@")
        || lowered.contains("authorization:")
    {
        return true;
    }
    let digit_count = trimmed.chars().filter(|character| character.is_ascii_digit()).count();
    if digit_count >= 9 {
        return true;
    }
    trimmed.split_whitespace().any(|token| {
        token.len() >= 24
            && token
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    })
}

fn expected_control_type_id(value: &str) -> Result<UIA_CONTROLTYPE_ID, String> {
    match value {
        "Button" => Ok(UIA_ButtonControlTypeId),
        "Edit" => Ok(UIA_EditControlTypeId),
        "Hyperlink" => Ok(UIA_HyperlinkControlTypeId),
        "ListItem" => Ok(UIA_ListItemControlTypeId),
        "MenuItem" => Ok(UIA_MenuItemControlTypeId),
        "TabItem" => Ok(UIA_TabItemControlTypeId),
        "Text" => Ok(UIA_TextControlTypeId),
        _ => Err("Desktop plan requested an unsupported UI control type".to_string()),
    }
}

fn is_absolute_windows_exe(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() > 7
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes[2] == b'\\'
        && value.to_ascii_lowercase().ends_with(".exe")
}

fn contains_sensitive_term(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    SENSITIVE_TERMS.iter().any(|term| lowered.contains(term))
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
