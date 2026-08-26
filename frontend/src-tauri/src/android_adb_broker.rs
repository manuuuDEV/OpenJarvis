//! Native, approval-gated Android ADB software diagnostics.
//!
//! This module deliberately exposes no general shell, input events, package
//! installation, file transfer, port forwarding, wireless pairing, root, or
//! app-launch interface. It can run only a fixed read-only probe set against
//! the one Android serial configured locally by the user.

use std::time::Duration;

use getrandom::fill;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, timeout};

use super::{read_android_adb_config, AndroidAdbConfig};

const BROKER_HEADER: &str = "x-openjarvis-android-adb-broker";
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_OUTPUT_BYTES: usize = 128 * 1024;
const MAX_VALUE_CHARS: usize = 80;

#[derive(Debug, Deserialize)]
struct ApprovedDiagnosticIds {
    ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ClaimedDiagnostic {
    id: String,
    plan: DiagnosticPlan,
}

#[derive(Debug, Deserialize)]
struct DiagnosticPlan {
    version: u8,
    summary: String,
}

#[derive(Debug, Serialize)]
struct DiagnosticCompletion<'a> {
    success: bool,
    summary: &'a str,
}

/// Generate a fresh 256-bit capability token per desktop launch.
pub fn launch_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    fill(&mut bytes).map_err(|error| format!("Could not create Android ADB broker secret: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Start the local Android diagnostic worker after the Python server is ready.
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
            .get(format!("{api_base}/v1/approvals/android-adb/approved"))
            .send()
            .await;
        let response = match listed {
            Ok(response) if response.status().is_success() => response,
            Ok(response) if response.status().as_u16() == 403 => return,
            _ => continue,
        };
        let approved = match response.json::<ApprovedDiagnosticIds>().await {
            Ok(approved) => approved,
            Err(_) => continue,
        };
        for action_id in approved.ids {
            let claimed = client
                .post(format!(
                    "{api_base}/v1/approvals/{action_id}/android-adb/claim"
                ))
                .send()
                .await;
            let response = match claimed {
                Ok(response) if response.status().is_success() => response,
                _ => continue,
            };
            let claimed = match response.json::<ClaimedDiagnostic>().await {
                Ok(claimed) => claimed,
                Err(_) => continue,
            };
            let action_id = claimed.id;
            let outcome = execute_diagnostic(claimed.plan).await;
            let (success, summary) = match outcome {
                Ok(summary) => (true, summary),
                Err(error) => (false, error),
            };
            let completion = DiagnosticCompletion {
                success,
                summary: &summary,
            };
            let _ = client
                .post(format!(
                    "{api_base}/v1/approvals/{action_id}/android-adb/complete"
                ))
                .json(&completion)
                .send()
                .await;
        }
    }
}

async fn execute_diagnostic(plan: DiagnosticPlan) -> Result<String, String> {
    if plan.version != 1 || plan.summary.trim().is_empty() || plan.summary.len() > 240 {
        return Err("Android diagnostic plan is invalid".to_string());
    }
    let config = read_android_adb_config();
    validate_config(&config)?;

    let state = adb_output(&config, &["get-state"]).await?;
    if state.trim() != "device" {
        return Err("The locally selected Android device is not authorized and ready".to_string());
    }

    // Fixed read-only probes. No plan field can add, remove, or alter arguments.
    let android_version = adb_shell(&config, &["getprop", "ro.build.version.release"]).await?;
    let manufacturer = adb_shell(&config, &["getprop", "ro.product.manufacturer"]).await?;
    let model = adb_shell(&config, &["getprop", "ro.product.model"]).await?;
    let screen_size = adb_shell(&config, &["wm", "size"]).await?;
    let storage = adb_shell(&config, &["df", "/data"]).await?;
    let memory = adb_shell(&config, &["dumpsys", "meminfo"]).await?;
    let battery = adb_shell(&config, &["dumpsys", "battery"]).await?;
    let packages = adb_shell(&config, &["pm", "list", "packages", "-3"]).await?;

    Ok(build_report(
        &android_version,
        &manufacturer,
        &model,
        &screen_size,
        &storage,
        &memory,
        &battery,
        &packages,
    ))
}

fn validate_config(config: &AndroidAdbConfig) -> Result<(), String> {
    if !config.diagnostics_acknowledged {
        return Err("Android ADB diagnostics have not been authorized in Settings".to_string());
    }
    let path = config
        .adb_path
        .as_deref()
        .ok_or_else(|| "No Android Platform Tools adb.exe path is configured".to_string())?;
    let serial = config
        .device_serial
        .as_deref()
        .ok_or_else(|| "No Android device is selected in Settings".to_string())?;
    if !super::is_safe_android_adb_path(path) || !std::path::Path::new(path).is_file() {
        return Err("Configured adb.exe path is not an approved Android Platform Tools binary".to_string());
    }
    if !super::is_safe_android_adb_serial(serial) {
        return Err("Configured Android device identity is invalid".to_string());
    }
    Ok(())
}

async fn adb_shell(config: &AndroidAdbConfig, shell_args: &[&str]) -> Result<String, String> {
    let mut args = vec!["shell"];
    args.extend_from_slice(shell_args);
    adb_output(config, &args).await
}

async fn adb_output(config: &AndroidAdbConfig, args: &[&str]) -> Result<String, String> {
    let path = config.adb_path.as_deref().unwrap_or_default();
    let serial = config.device_serial.as_deref().unwrap_or_default();
    let mut command = tokio::process::Command::new(path);
    command.arg("-s").arg(serial).args(args);
    command.kill_on_drop(true);
    let output = timeout(COMMAND_TIMEOUT, command.output())
        .await
        .map_err(|_| "Android diagnostic command timed out".to_string())?
        .map_err(|_| "Android Platform Tools could not run the diagnostic command".to_string())?;
    if !output.status.success() {
        return Err("Android diagnostic command was rejected by the device".to_string());
    }
    if output.stdout.len() > MAX_OUTPUT_BYTES {
        return Err("Android diagnostic output exceeded its safety limit".to_string());
    }
    String::from_utf8(output.stdout)
        .map_err(|_| "Android diagnostic output was not valid text".to_string())
}

fn build_report(
    android_version: &str,
    manufacturer: &str,
    model: &str,
    screen_size: &str,
    storage: &str,
    memory: &str,
    battery: &str,
    packages: &str,
) -> String {
    let mut lines = vec!["Android software diagnostic completed.".to_string()];
    let version = safe_value(android_version);
    let maker = safe_value(manufacturer);
    let device_model = safe_value(model);
    if !version.is_empty() {
        lines.push(format!("Android version: {version}."));
    }
    if !maker.is_empty() || !device_model.is_empty() {
        lines.push(format!("Device product: {} {}.", maker, device_model).trim().to_string());
    }
    if let Some(size) = screen_size.lines().find(|line| line.contains("Physical size")) {
        lines.push(format!("Display: {}.", safe_value(size)));
    }
    if let Some(data_line) = storage.lines().skip(1).find(|line| line.contains("/data")) {
        let fields: Vec<&str> = data_line.split_whitespace().collect();
        if fields.len() >= 5 {
            lines.push(format!(
                "Data storage: total {}, used {}, available {} ({} used).",
                fields[1], fields[2], fields[3], fields[4]
            ));
            if let Some(percent) = fields[4].strip_suffix('%').and_then(|value| value.parse::<u8>().ok()) {
                if percent >= 90 {
                    lines.push("Potential issue: data storage is above 90% used.".to_string());
                }
            }
        }
    }
    if let Some(total_ram) = memory.lines().find(|line| line.trim_start().starts_with("Total RAM:")) {
        lines.push(format!("Memory: {}.", safe_value(total_ram)));
    }
    let level = field_value(battery, "level:");
    let status = field_value(battery, "status:");
    if !level.is_empty() || !status.is_empty() {
        lines.push(format!("Battery: level {}%, status {}.", level, status));
        if level.parse::<u8>().is_ok_and(|value| value <= 15) {
            lines.push("Potential issue: battery level is low.".to_string());
        }
    }
    let package_count = packages.lines().filter(|line| line.starts_with("package:")).count();
    lines.push(format!("Third-party application count: {package_count}."));
    lines.join("\n")
}

fn field_value(text: &str, label: &str) -> String {
    text.lines()
        .find_map(|line| line.trim().strip_prefix(label))
        .map(safe_value)
        .unwrap_or_default()
}

fn safe_value(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_VALUE_CHARS)
        .collect()
}
