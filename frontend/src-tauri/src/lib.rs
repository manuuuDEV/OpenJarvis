use std::sync::Arc;
#[cfg(target_os = "windows")]
mod android_adb_broker;
#[cfg(target_os = "windows")]
mod desktop_broker;

use std::time::Duration;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
use tokio::sync::Mutex;

const OLLAMA_PORT: u16 = 11434;
const JARVIS_PORT: u16 = 8000;
const DESKTOP_UV_SYNC_COMMAND: &str =
    "uv sync --extra server --extra inference-cloud --extra inference-google --group desktop-native";

/// Small, fast model used when startup needs a default Ollama tag.
const STARTUP_MODEL: &str = "qwen3.5:4b";

/// Tiny fallback model if even the startup model can't be pulled.
const FALLBACK_MODEL: &str = "qwen3:0.6b";

/// Qwen3.5 model variants, ordered smallest to largest.
/// Each entry is (ollama_tag, approximate_download_size_gb, min_ram_gb).
const QWEN35_MODELS: &[(&str, f64, f64)] = &[
    ("qwen3.5:0.8b", 1.0, 4.0),
    ("qwen3.5:2b", 2.7, 6.0),
    ("qwen3.5:4b", 3.4, 8.0),
    ("qwen3.5:9b", 6.6, 12.0),
    ("qwen3.5:27b", 17.0, 24.0),
    ("qwen3.5:35b", 24.0, 32.0),
    ("qwen3.5:122b", 81.0, 96.0),
];

/// Get total system RAM in GB.
fn total_ram_gb() -> f64 {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
            if let Ok(s) = String::from_utf8(output.stdout) {
                if let Ok(bytes) = s.trim().parse::<u64>() {
                    return bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = std::fs::read_to_string("/proc/meminfo") {
            for line in contents.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return kb as f64 / (1024.0 * 1024.0);
                        }
                    }
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        // wmic returns TotalVisibleMemorySize in KB
        if let Ok(output) = Command::new("wmic")
            .args(["OS", "get", "TotalVisibleMemorySize", "/value"])
            .output()
        {
            if let Ok(s) = String::from_utf8(output.stdout) {
                for line in s.lines() {
                    if let Some(val) = line.strip_prefix("TotalVisibleMemorySize=") {
                        if let Ok(kb) = val.trim().parse::<u64>() {
                            return kb as f64 / (1024.0 * 1024.0);
                        }
                    }
                }
            }
        }
    }
    8.0
}

/// Return the Qwen3.5 models that fit in `ram_gb`, smallest first.
fn models_that_fit_in(ram_gb: f64) -> Vec<&'static str> {
    QWEN35_MODELS
        .iter()
        .filter(|(_, _, min_ram)| ram_gb >= *min_ram)
        .map(|(tag, _, _)| *tag)
        .collect()
}

/// The default local model: the second-largest Qwen3.5 model that fits in
/// `ram_gb`. Falls back to the only fitting model, or FALLBACK_MODEL if none
/// fit. Deliberately NOT the largest — leaves RAM headroom for the OS/app.
fn default_local_model(ram_gb: f64) -> &'static str {
    let fitting = models_that_fit_in(ram_gb);
    match fitting.len() {
        0 => FALLBACK_MODEL,
        1 => fitting[0],
        n => fitting[n - 2],
    }
}

/// A resolved boot plan derived purely from the inference config + RAM.
/// Pure and side-effect-free so it can be unit-tested without spawning
/// processes or touching the network.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BootPlan {
    /// Whether to start and wait for the bundled Ollama.
    launch_ollama: bool,
    /// The preferred Ollama model (None for custom endpoints).
    model_to_pull: Option<String>,
    /// Optional `(engine_key, bare_host)` override for a custom endpoint,
    /// e.g. `("lmstudio", "http://localhost:1234")`. Written into
    /// ~/.openjarvis/config.toml so `jarvis serve` picks it up.
    engine_host: Option<(String, String)>,
    /// Args appended after `uv run jarvis serve --port <port>`.
    serve_args: Vec<String>,
}

/// Default OpenAI-compatible engine key used when a custom endpoint config
/// omits one (LM Studio is the canonical local server).
const CUSTOM_FALLBACK_ENGINE: &str = "lmstudio";
// Only providers explicitly exposed by the secure desktop settings are accepted.
// A profile selects one provider at a time; this list never enables fallback.
const CLOUD_PROVIDERS: &[&str] = &[
    "openai",
    "google",
    "openrouter",
    "groq",
    "nvidia",
    "sambanova",
    "alibaba",
    "pollinations",
    "huggingface",
    "together",
];

fn cloud_api_key_name(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" => Some("OPENAI_API_KEY"),
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "google" => Some("GEMINI_API_KEY"),
        "openrouter" => Some("OPENROUTER_API_KEY"),
        "groq" => Some("GROQ_API_KEY"),
        "nvidia" => Some("NVIDIA_API_KEY"),
        "sambanova" => Some("SAMBANOVA_API_KEY"),
        "alibaba" => Some("DASHSCOPE_API_KEY"),
        "pollinations" => Some("POLLINATIONS_API_KEY"),
        "huggingface" => Some("HF_TOKEN"),
        "together" => Some("TOGETHER_API_KEY"),
        _ => None,
    }
}

fn validate_cloud_provider(provider: &str) -> Result<(), String> {
    if CLOUD_PROVIDERS.contains(&provider) {
        Ok(())
    } else {
        Err(format!("Unsupported cloud provider: {:?}", provider))
    }
}

fn provider_endpoint_required(provider: &str) -> bool {
    matches!(provider, "sambanova" | "alibaba")
}

fn validate_provider_endpoint(provider: &str, endpoint: Option<&str>) -> Result<Option<String>, String> {
    let endpoint = endpoint.map(str::trim).filter(|value| !value.is_empty());
    if endpoint.is_none() && provider_endpoint_required(provider) {
        return Err("This provider requires its HTTPS endpoint from your provider console.".into());
    }
    let Some(endpoint) = endpoint else {
        // Pollinations has one documented canonical HTTPS base; no custom
        // endpoint is exposed in Settings for this provider.
        return if provider == "pollinations" {
            Ok(Some("https://gen.pollinations.ai".to_string()))
        } else {
            Ok(None)
        };
    };
    let url = reqwest::Url::parse(endpoint)
        .map_err(|_| "Provider endpoint is not a valid URL.".to_string())?;
    if url.scheme() != "https" || url.query().is_some() || url.fragment().is_some() {
        return Err("Provider endpoint must be a clean HTTPS base URL.".into());
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let allowed = match provider {
        "sambanova" => host == "api.sambanova.ai" || host.ends_with(".sambanova.ai"),
        "alibaba" => host.ends_with(".aliyuncs.com"),
        "pollinations" => host == "gen.pollinations.ai",
        _ => false,
    };
    if !allowed {
        return Err("Provider endpoint host is not authorized for this profile.".into());
    }
    Ok(Some(endpoint.trim_end_matches('/').to_string()))
}

/// Decide what to launch/pull/serve from the inference config + system RAM.
/// Pure: no I/O, no spawning.
fn boot_plan(cfg: &InferenceConfig, ram_gb: f64) -> BootPlan {
    match cfg.kind {
        SourceKind::Ollama => {
            let model = cfg
                .model
                .clone()
                .unwrap_or_else(|| default_local_model(ram_gb).to_string());
            BootPlan {
                launch_ollama: true,
                model_to_pull: Some(model.clone()),
                engine_host: None,
                serve_args: vec![
                    "--engine".into(),
                    "ollama".into(),
                    "--model".into(),
                    model,
                    "--agent".into(),
                    "simple".into(),
                ],
            }
        }
        SourceKind::Cloud => {
            let model = cfg.model.clone().unwrap_or_default();
            BootPlan {
                launch_ollama: false,
                model_to_pull: None,
                engine_host: None,
                serve_args: vec![
                    "--engine".into(),
                    "cloud".into(),
                    "--model".into(),
                    model,
                    "--agent".into(),
                    "simple".into(),
                ],
            }
        }
        SourceKind::Custom => {
            let engine = cfg
                .engine
                .clone()
                .unwrap_or_else(|| CUSTOM_FALLBACK_ENGINE.to_string());
            // Record (engine_key, bare_host) only when a host is configured, so
            // boot can write `[engine.<key>] host = ...` into config.toml. An
            // empty host is dropped (no override).
            let engine_host = cfg
                .host
                .clone()
                .filter(|h| !h.is_empty())
                .map(|h| (engine.clone(), h));
            // `model` may be empty if the config is malformed; `jarvis serve`
            // surfaces a clear error then (there is no universal default model
            // for an arbitrary endpoint).
            let model = cfg.model.clone().unwrap_or_default();
            BootPlan {
                launch_ollama: false,
                model_to_pull: None,
                engine_host,
                serve_args: vec![
                    "--engine".into(),
                    engine,
                    "--model".into(),
                    model,
                    "--agent".into(),
                    "simple".into(),
                ],
            }
        }
    }
}

/// Get the user home directory, handling both Unix (HOME) and Windows (USERPROFILE).
fn home_dir() -> String {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default()
}

/// Resolve full path to a binary by checking common locations.
/// macOS .app bundles don't inherit the shell PATH, so we probe manually.
fn resolve_bin(name: &str) -> String {
    let home = home_dir();

    #[cfg(not(target_os = "windows"))]
    let candidates = vec![
        format!("/opt/homebrew/bin/{name}"),
        format!("{home}/.local/bin/{name}"),
        format!("{home}/.cargo/bin/{name}"),
        format!("/usr/local/bin/{name}"),
        format!("/usr/bin/{name}"),
    ];

    #[cfg(target_os = "windows")]
    let candidates = {
        let localappdata = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let programfiles = std::env::var("ProgramFiles").unwrap_or_default();
        let programfiles_x86 = std::env::var("ProgramFiles(x86)").unwrap_or_default();
        vec![
            // Git for Windows — standard install paths
            format!("{programfiles}\\Git\\cmd\\{name}.exe"),
            format!("{programfiles_x86}\\Git\\cmd\\{name}.exe"),
            format!("{localappdata}\\Programs\\Git\\cmd\\{name}.exe"),
            // Scoop package manager
            format!("{home}\\scoop\\shims\\{name}.exe"),
            // Cargo, local bin
            format!("{home}\\.cargo\\bin\\{name}.exe"),
            format!("{home}\\.local\\bin\\{name}.exe"),
            // Generic program locations
            format!("{localappdata}\\Programs\\{name}\\{name}.exe"),
            format!("{programfiles}\\{name}\\{name}.exe"),
            // Ollama installs to LOCALAPPDATA on Windows
            format!("{localappdata}\\Programs\\Ollama\\{name}.exe"),
            // uv installs via pip/pipx
            format!("{home}\\AppData\\Roaming\\Python\\Scripts\\{name}.exe"),
        ]
    };

    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return path.clone();
        }
    }

    // Fallback: ask the OS to find it on PATH.
    // On Windows this uses `where.exe`, on Unix `which`.
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("where")
            .arg(format!("{name}.exe"))
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = stdout.lines().next() {
                    let p = first_line.trim();
                    if !p.is_empty() && std::path::Path::new(p).exists() {
                        return p.to_string();
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(output) = std::process::Command::new("which").arg(name).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = stdout.lines().next() {
                    let p = first_line.trim();
                    if !p.is_empty() && std::path::Path::new(p).exists() {
                        return p.to_string();
                    }
                }
            }
        }
    }

    name.to_string()
}

/// Find the reviewed backend source embedded by the secure desktop installer.
///
/// Development may opt in to a local source directory, but release builds never
/// scan home directories or silently execute a nearby clone.
fn find_project_root() -> Option<std::path::PathBuf> {
    // Explicit development-only override. The installed profile never sets the
    // enabling flag, so a user environment variable cannot replace the bundled
    // backend with arbitrary local source during normal desktop use.
    if std::env::var("OPENJARVIS_ALLOW_DEVELOPMENT_SOURCE").ok().as_deref() == Some("1") {
        if let Ok(root) = std::env::var("OPENJARVIS_ROOT") {
            let path = std::path::PathBuf::from(&root);
            if path.join("pyproject.toml").exists() {
                return Some(path);
            }
        }
    }

    // The source tree embedded by the secure desktop installer.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let bundled_source = exe_dir.join("resources").join("openjarvis-source");
            if bundled_source.join("pyproject.toml").exists() {
                return Some(bundled_source);
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// BackendManager — owns the Ollama + Jarvis server child processes
// ---------------------------------------------------------------------------

struct ChildHandle {
    child: tokio::process::Child,
}

impl ChildHandle {
    async fn kill(&mut self) {
        let _ = self.child.kill().await;
    }
}

/// Rolling buffer holding the most recent ~16 KB of jarvis stderr.
///
/// Populated by a background drainer task spawned at boot so the pipe
/// never fills and back-pressures `jarvis serve`; consumed by the boot
/// path when surfacing failure messages.
type StderrTail = Arc<Mutex<Vec<u8>>>;

const STDERR_TAIL_LIMIT: usize = 16 * 1024;

struct BackendManager {
    ollama: Option<ChildHandle>,
    jarvis: Option<ChildHandle>,
    jarvis_stderr_tail: StderrTail,
}

impl Default for BackendManager {
    fn default() -> Self {
        Self {
            ollama: None,
            jarvis: None,
            jarvis_stderr_tail: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl BackendManager {
    async fn stop_all(&mut self) {
        if let Some(ref mut h) = self.jarvis {
            h.kill().await;
        }
        self.jarvis = None;
        if let Some(ref mut h) = self.ollama {
            h.kill().await;
        }
        self.ollama = None;
    }
}

type SharedBackend = Arc<Mutex<BackendManager>>;

// ---------------------------------------------------------------------------
// Setup status (reported to frontend)
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, Clone)]
struct SetupStatus {
    phase: String,
    detail: String,
    ollama_ready: bool,
    server_ready: bool,
    model_ready: bool,
    error: Option<String>,
    /// "ollama" | "custom" — lets the setup UI relabel the progress steps.
    source: String,
}

impl Default for SetupStatus {
    fn default() -> Self {
        Self {
            phase: "starting".into(),
            detail: "Initializing...".into(),
            ollama_ready: false,
            server_ready: false,
            model_ready: false,
            error: None,
            source: "ollama".into(),
        }
    }
}

type SharedStatus = Arc<Mutex<SetupStatus>>;

// ---------------------------------------------------------------------------
// Health-check helpers
// ---------------------------------------------------------------------------

async fn wait_for_url(url: &str, timeout: Duration) -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if let Ok(resp) = client.get(url).send().await {
            if resp.status().is_success() {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

/// True if a custom OpenAI-compatible endpoint answers at all (any HTTP
/// status counts — even a 404 proves the server is up). `host` is the bare
/// base URL; we probe `<host>/v1/models`.
async fn endpoint_reachable(host: &str, timeout: Duration) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    let url = format!("{}/v1/models", host.trim_end_matches('/'));
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if client.get(&url).send().await.is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

/// Outcome of waiting for `jarvis serve` to become healthy.
///
/// Unlike [`wait_for_url`] this differentiates "server is up but degraded"
/// (HTTP 503 — usually inference engine failed to load) from "server never
/// came up" and from "child process died before serving anything", because
/// each needs a different user-facing message.
#[derive(Debug)]
enum JarvisStartResult {
    /// `/health` returned 2xx.
    Ready,
    /// Server replied 503. The body is the actionable message (typically
    /// "engine not ready" or a model-load error).
    ServiceUnavailable(String),
    /// The `jarvis serve` child exited before `/health` returned 2xx.
    EarlyExit { code: Option<i32>, stderr: String },
    /// Deadline elapsed without ever seeing 2xx or an early exit.
    Timeout,
}

/// Spawn a detached task that continuously drains `jarvis serve`'s
/// stderr into a rolling tail buffer.
///
/// We MUST keep reading stderr for as long as the child runs — `jarvis
/// serve` is chatty (engine load progress, request logs), and the OS
/// pipe buffer is small (4 KB on Windows, 64 KB on Linux). Once full,
/// the child's next stderr write blocks indefinitely and the server
/// hangs mid-operation. The drainer reads in chunks and keeps only the
/// last `STDERR_TAIL_LIMIT` bytes — enough to surface a tail trace if
/// the child later dies, without unbounded memory growth.
///
/// Returns immediately after spawning the task; the task ends naturally
/// when the child closes stderr (i.e. exits).
fn spawn_jarvis_stderr_drainer(mut stderr: tokio::process::ChildStderr, tail: StderrTail) {
    use tokio::io::AsyncReadExt;
    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            match stderr.read(&mut buf).await {
                Ok(0) => break,  // EOF — child closed stderr
                Err(_) => break, // pipe broke — also done
                Ok(n) => {
                    let mut t = tail.lock().await;
                    t.extend_from_slice(&buf[..n]);
                    if t.len() > STDERR_TAIL_LIMIT {
                        let drop_n = t.len() - STDERR_TAIL_LIMIT;
                        t.drain(..drop_n);
                    }
                }
            }
        }
    });
}

/// Read whatever the stderr drainer has buffered so far.
///
/// Safe to call at any time; returns an empty string before the
/// drainer has seen any bytes. Trimmed.
async fn read_jarvis_stderr_tail(backend: &SharedBackend) -> String {
    let tail = backend.lock().await.jarvis_stderr_tail.clone();
    let bytes = tail.lock().await.clone();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

/// Poll `jarvis serve` health, watching the child process state so we
/// never wait 10 minutes for a process that crashed in the first second.
async fn wait_for_jarvis_health(
    url: &str,
    timeout: Duration,
    backend: &SharedBackend,
) -> JarvisStartResult {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(_) => return JarvisStartResult::Timeout,
    };
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        // 1. Has the child already exited? `try_wait` is non-blocking; on
        // Windows where uv / python / the Rust extension can fail to load
        // very fast, this catches the crash within ~500ms instead of after
        // the full HTTP timeout window.
        let exit_status = {
            let mut mgr = backend.lock().await;
            match mgr.jarvis.as_mut() {
                Some(h) => h.child.try_wait().ok().flatten(),
                None => None,
            }
        };
        if let Some(status) = exit_status {
            let stderr = read_jarvis_stderr_tail(backend).await;
            return JarvisStartResult::EarlyExit {
                code: status.code(),
                stderr,
            };
        }

        // 2. Try the health endpoint.
        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return JarvisStartResult::Ready;
                }
                if status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
                    // Server is up but the inference engine is not. This
                    // is a terminal-for-us state — polling won't change
                    // anything; the user has to fix their engine config.
                    let body = resp.text().await.unwrap_or_default();
                    return JarvisStartResult::ServiceUnavailable(body);
                }
                // Other non-2xx (e.g. 404 during a brief routing-table
                // warmup window) — fall through and keep polling.
            }
            Err(_) => {
                // Connection refused / DNS / timeout — server still
                // booting. Keep polling.
            }
        }

        if tokio::time::Instant::now() >= deadline {
            return JarvisStartResult::Timeout;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn ollama_has_model(model: &str) -> bool {
    let models = ollama_model_names().await;
    matching_installed_model(&models, model).is_some()
}

fn parse_ollama_model_names(body: &serde_json::Value) -> Vec<String> {
    body.get("models")
        .and_then(|m| m.as_array())
        .map(|models| {
            models
                .iter()
                .filter_map(|m| {
                    m.get("name")
                        .or_else(|| m.get("model"))
                        .and_then(|n| n.as_str())
                })
                .filter(|name| !name.trim().is_empty())
                .map(|name| name.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn model_names_match(installed: &str, requested: &str) -> bool {
    installed == requested
        || installed.strip_suffix(":latest") == Some(requested)
        || requested.strip_suffix(":latest") == Some(installed)
}

fn matching_installed_model(models: &[String], requested: &str) -> Option<String> {
    models
        .iter()
        .find(|model| model_names_match(model, requested))
        .cloned()
}

fn model_name_looks_embedding_only(model: &str) -> bool {
    let name = model.to_ascii_lowercase();
    ["embed", "embedding", "rerank", "minilm", "bge-", "bge_", "e5-", "e5_"]
        .iter()
        .any(|marker| name.contains(marker))
}

fn preferred_installed_model(models: &[String]) -> Option<String> {
    models
        .iter()
        .find(|model| !model.trim().is_empty() && !model_name_looks_embedding_only(model))
        .or_else(|| models.iter().find(|model| !model.trim().is_empty()))
        .cloned()
}

fn startup_installed_model(requested_model: &str, installed_models: &[String]) -> Option<String> {
    matching_installed_model(installed_models, requested_model)
        .or_else(|| preferred_installed_model(installed_models))
}

fn should_persist_resolved_model(cfg: &InferenceConfig) -> bool {
    cfg.model
        .as_deref()
        .map(|model| model.trim().is_empty())
        .unwrap_or(true)
}

async fn ollama_model_names() -> Vec<String> {
    let url = format!("http://127.0.0.1:{}/api/tags", OLLAMA_PORT);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    if let Ok(resp) = client.get(&url).send().await {
        if let Ok(body) = resp.json::<serde_json::Value>().await {
            return parse_ollama_model_names(&body);
        }
    }
    Vec::new()
}

async fn pull_model(model: &str) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/api/pull", OLLAMA_PORT);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(&url)
        .json(&serde_json::json!({"name": model, "stream": false}))
        .send()
        .await
        .map_err(|e| format!("Pull request failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Pull returned status {}", resp.status()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// uv sync error formatting (pure helpers — unit-tested, see #331)
// ---------------------------------------------------------------------------

/// Last `max_chars` characters of a `uv sync` stderr stream, trimmed.
///
/// uv's actionable diagnostic almost always lands at the tail of the
/// stream, so when surfacing a failure to the user we show the end, not
/// the (usually noisy progress-spinner) beginning. Operates on `char`
/// boundaries so it never splits a multi-byte UTF-8 codepoint — important
/// because Windows consoles emit non-ASCII (cp9xx) bytes.
fn uv_sync_stderr_tail(stderr: &str, max_chars: usize) -> String {
    let total = stderr.chars().count();
    let skip = total.saturating_sub(max_chars);
    stderr.chars().skip(skip).collect::<String>().trim().to_string()
}

/// Error message shown when `uv sync` runs but exits non-zero (#331).
///
/// `exit_code` is `None` when the process was terminated by a signal with
/// no exit code (rendered as "unknown" rather than a misleading -1).
fn format_uv_sync_failure(
    root: &std::path::Path,
    exit_code: Option<i32>,
    stderr: &str,
) -> String {
    let code = exit_code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let tail = uv_sync_stderr_tail(stderr, 800);
    let rust_hint = if looks_like_rust_extension_build_error(stderr) {
        format!("\n\n{}", rust_toolchain_install_hint())
    } else {
        String::new()
    };
    format!(
        "`uv sync` failed in {} (exit {}). Last output:\n\n{}\n\n\
         Try opening a terminal in that directory and running \
         `{}` manually for the full output.{}",
        root.display(),
        code,
        tail,
        DESKTOP_UV_SYNC_COMMAND,
        rust_hint,
    )
}

/// Strip AppImage-injected environment from a subprocess command (#455).
///
/// When the OpenJarvis desktop binary is shipped as an AppImage, the AppImage
/// runtime sets `LD_LIBRARY_PATH` (and friends) to the extracted-to-/tmp
/// bundled lib dir. Any child we spawn inherits that env by default — but the
/// children we spawn (`uv`, `ollama`, `git`) live outside the AppImage and
/// must NOT load their shared libraries from the AppImage's bundle. The
/// classic symptom: `uv` finds `python3`, `python3` tries to `import numpy`,
/// numpy's `.so` files try to dlopen libstdc++/libssl/libcrypto, the linker
/// picks the AppImage's versions which were built against a different glibc
/// or libcrypto API, and python dies silently — before any startup log
/// reaches us. The user sees "API Server — starting server..." forever.
///
/// Fix: when we detect we're inside an AppImage (the AppImage runtime sets
/// `$APPIMAGE` to the original image path), strip the leaked env vars before
/// spawn. Conditional on `APPIMAGE` being set so regular Linux installs that
/// legitimately use `LD_LIBRARY_PATH` are untouched. Linux-only — the
/// `#[cfg]` makes this a no-op on macOS / Windows.
#[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
fn prepare_subprocess_for_appimage(cmd: &mut tokio::process::Command) {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("APPIMAGE").is_some() {
            cmd.env_remove("LD_LIBRARY_PATH");
            cmd.env_remove("LD_PRELOAD");
            cmd.env_remove("APPIMAGE");
            cmd.env_remove("APPIMAGE_UUID");
            cmd.env_remove("APPDIR");
            cmd.env_remove("ARGV0");
        }
    }
}

/// Error message shown when `uv sync` can't even be spawned (#331) —
/// e.g. the resolved `uv` binary doesn't exist or isn't executable.
fn format_uv_sync_spawn_error(root: &std::path::Path, uv_bin: &str, err: &str) -> String {
    format!(
        "Could not run `uv sync`: {}. Verify uv is installed at \
         `{}` and the OpenJarvis repo is at `{}`.",
        err,
        uv_bin,
        root.display(),
    )
}

fn rust_toolchain_install_hint() -> &'static str {
    "The desktop app needs the Rust toolchain to build `openjarvis_rust`. \
     Install Rust from https://rustup.rs. On Windows, also install Visual Studio \
     Build Tools with the C++ workload, then relaunch."
}

fn looks_like_rust_extension_build_error(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    [
        "openjarvis-rust",
        "openjarvis_rust",
        "maturin",
        "cargo",
        "rustc",
        "link.exe",
        "visual studio",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn format_missing_rust_toolchain() -> String {
    format!(
        "Could not find Rust's `cargo` command. {}\n\n\
         If Rust is already installed, close and relaunch the desktop app so \
         PATH includes `~/.cargo/bin`.",
        rust_toolchain_install_hint(),
    )
}

fn format_extension_import_failure(root: &std::path::Path, stderr: &str) -> String {
    let tail = uv_sync_stderr_tail(stderr, 4000);
    format!(
        "`openjarvis_rust` is still not importable after building. Last output:\n\n{}\n\n\
         Run these manually for the full build log:\n\n\
           cd {}\n\
           {}\n\
           uv run python -c \"import openjarvis_rust\"",
        if tail.is_empty() {
            "(no stderr output)"
        } else {
            &tail
        },
        root.display(),
        DESKTOP_UV_SYNC_COMMAND,
    )
}

fn add_cargo_bin_to_path(cmd: &mut tokio::process::Command) {
    let mut paths: Vec<std::path::PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    paths.insert(
        0,
        std::path::PathBuf::from(home_dir())
            .join(".cargo")
            .join("bin"),
    );
    if let Ok(joined) = std::env::join_paths(paths) {
        cmd.env("PATH", joined);
    }
}

async fn verify_openjarvis_rust_extension(
    root: &std::path::Path,
    uv_bin: &str,
) -> Result<(), String> {
    let mut cmd = tokio::process::Command::new(uv_bin);
    cmd.args(["run", "python", "-c", "import openjarvis_rust"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .current_dir(root);
    prepare_subprocess_for_appimage(&mut cmd);
    add_cargo_bin_to_path(&mut cmd);

    match cmd.output().await {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(format_extension_import_failure(root, &stderr))
        }
        Err(e) => Err(format!(
            "Could not verify `openjarvis_rust`: {}. Verify uv is installed at `{}`.",
            e, uv_bin
        )),
    }
}

fn port_owner_hint() -> String {
    if cfg!(target_os = "windows") {
        format!("netstat -ano | findstr :{}", JARVIS_PORT)
    } else {
        format!("lsof -i :{}", JARVIS_PORT)
    }
}

fn format_port_unavailable(port: u16, reason: &str) -> String {
    format!(
        "Port {} is not available: {}. Stop the process using that port or \
         change the OpenJarvis port, then relaunch.\n\nTo identify it:\n  {}",
        port,
        reason,
        port_owner_hint(),
    )
}

fn check_jarvis_port_available() -> Result<(), String> {
    match std::net::TcpListener::bind(("127.0.0.1", JARVIS_PORT)) {
        Ok(listener) => {
            drop(listener);
            Ok(())
        }
        Err(err) => Err(format_port_unavailable(JARVIS_PORT, &err.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Backend boot sequence (runs in background after app launch)
// ---------------------------------------------------------------------------

async fn boot_backend(backend: SharedBackend, status: SharedStatus) {
    // Decide the inference source before launching anything. Cloud is the
    // default kind when no config is present, but it must not block the local
    // server: the provider is only consulted for actual cloud requests.
    let cfg = read_inference_config();
    let plan = boot_plan(&cfg, total_ram_gb());
    {
        let mut s = status.lock().await;
        s.source = match cfg.kind {
            SourceKind::Ollama => "ollama",
            SourceKind::Custom => "custom",
            SourceKind::Cloud => "cloud",
        }
        .into();
    }

    // For the Ollama path, model resolution may fall back to FALLBACK_MODEL; we
    // record what is actually available here so the serve command below uses
    // it instead of the originally-planned tag. None on the custom path.
    let mut serve_model_override: Option<String> = None;

    if plan.launch_ollama {
        // Phase 1: Start Ollama
        {
            let mut s = status.lock().await;
            s.phase = "ollama".into();
            s.detail = "Starting inference engine...".into();
        }

        // Try the bundled sidecar first, fall back to system ollama
        let ollama_child = {
            let ollama_bin = resolve_bin("ollama");
            let mut sidecar_cmd = tokio::process::Command::new(&ollama_bin);
            sidecar_cmd
                .arg("serve")
                .env("OLLAMA_HOST", format!("127.0.0.1:{}", OLLAMA_PORT))
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            // Avoid LD_LIBRARY_PATH leak when running inside an AppImage (#455).
            prepare_subprocess_for_appimage(&mut sidecar_cmd);
            match sidecar_cmd.spawn() {
                Ok(child) => Some(child),
                Err(_) => None,
            }
        };

        if let Some(child) = ollama_child {
            backend.lock().await.ollama = Some(ChildHandle { child });
        }

        let ollama_url = format!("http://127.0.0.1:{}/api/tags", OLLAMA_PORT);
        if !wait_for_url(&ollama_url, Duration::from_secs(30)).await {
            let mut s = status.lock().await;
            s.error = Some("Could not start Ollama. Install it from https://ollama.com".into());
            return;
        }

        {
            let mut s = status.lock().await;
            s.ollama_ready = true;
            s.detail = "Inference engine ready.".into();
        }

        // Phase 2: Resolve one model to serve. Prefer an installed model on
        // first run so startup does not depend on a download succeeding.
        let model = plan
            .model_to_pull
            .clone()
            .unwrap_or_else(|| STARTUP_MODEL.to_string());
        {
            let mut s = status.lock().await;
            s.phase = "model".into();
            s.detail = format!("Checking for {}...", model);
        }

        let installed_models = ollama_model_names().await;
        let resolved_model = if let Some(installed) = startup_installed_model(&model, &installed_models) {
            installed
        } else {
            {
                let mut s = status.lock().await;
                s.detail = format!("Downloading {}... (this may take a minute)", model);
            }
            match pull_model(&model).await {
                Ok(()) => model.clone(),
                Err(e) => {
                    eprintln!("Warning: failed to pull {}: {}", model, e);

                    // If a local model appeared while pulling, use it instead of
                    // making startup depend on another network pull.
                    if let Some(installed) = preferred_installed_model(&ollama_model_names().await) {
                        installed
                    } else if ollama_has_model(FALLBACK_MODEL).await {
                        FALLBACK_MODEL.to_string()
                    } else {
                        {
                            let mut s = status.lock().await;
                            s.detail = format!("Downloading {}...", FALLBACK_MODEL);
                        }
                        if let Err(e2) = pull_model(FALLBACK_MODEL).await {
                            if let Some(installed) =
                                preferred_installed_model(&ollama_model_names().await)
                            {
                                installed
                            } else {
                                let mut s = status.lock().await;
                                s.error = Some(format!("Failed to download model: {}", e2));
                                return;
                            }
                        } else {
                            FALLBACK_MODEL.to_string()
                        }
                    }
                }
            }
        };

        if resolved_model != model {
            let mut s = status.lock().await;
            s.detail = format!("Using installed model {}.", resolved_model);
        }

        serve_model_override = Some(resolved_model.clone());

        // Persist only first-run/default resolution. If the user explicitly
        // configured a model, do not overwrite that choice with a temporary
        // fallback selected just to keep startup nonfatal.
        if should_persist_resolved_model(&cfg) {
            let mut persisted = cfg.clone();
            persisted.model = Some(resolved_model);
            let _ = write_inference_config(&persisted);
        }

        {
            let mut s = status.lock().await;
            s.model_ready = true;
            s.detail = "Model ready.".into();
        }
    } else if cfg.kind == SourceKind::Cloud {
        // Cloud is the default inference source, but it must never block the
        // local backend from starting. A fresh install with no configured
        // provider (or an explicit but not-yet-keyed provider) still boots
        // `jarvis serve` so the app is usable; the provider is only consulted
        // when a request actually needs the cloud path. We validate and
        // authorize a configured provider opportunistically, without ever
        // returning early and halting the server.
        let provider = cfg.provider.clone().unwrap_or_default();
        let model = cfg.model.clone().unwrap_or_default();
        if !provider.is_empty() {
            let mut block_reason: Option<String> = None;
            if model.is_empty() {
                block_reason = Some("Choose a cloud model in Settings before sending a cloud request.".into());
            } else if let Err(error) = validate_cloud_provider(&provider) {
                block_reason = Some(error);
            } else if cloud_api_key_name(&provider).is_none() {
                block_reason =
                    Some("The selected cloud provider has no supported credential mapping.".into());
            } else if !matches!(secure_store_get(
                cloud_api_key_name(&provider).expect("checked above")
            ), Ok(Some(value)) if !value.is_empty()) {
                block_reason = Some(format!(
                    "Add the API key for the authorized provider ({}) in Settings before sending a cloud request.",
                    provider
                ));
            }

            match block_reason {
                Some(reason) => {
                    let mut s = status.lock().await;
                    s.phase = "model".into();
                    s.ollama_ready = true;
                    s.model_ready = true;
                    s.detail = reason;
                }
                None => {
                    let _ = set_cloud_privacy_config(&provider);
                    let mut s = status.lock().await;
                    s.phase = "model".into();
                    s.ollama_ready = true;
                    s.model_ready = true;
                    s.detail =
                        format!("Cloud provider {} authorized with TLS required.", provider);
                }
            }
        } else {
            let mut s = status.lock().await;
            s.phase = "model".into();
            s.ollama_ready = true;
            s.model_ready = true;
            s.detail = "No cloud provider configured; local server is running.".into();
        }
    } else {
        // Legacy custom endpoint: never start Ollama, never download.
        let host = plan
            .engine_host
            .as_ref()
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        {
            let mut s = status.lock().await;
            s.phase = "model".into();
            s.detail = format!("Connecting to {}...", host);
        }
        if host.is_empty() || !endpoint_reachable(&host, Duration::from_secs(15)).await {
            let mut s = status.lock().await;
            s.error = Some(format!(
                "Could not reach your custom inference server at {}. \
                 Start the server (e.g. LM Studio) and check the URL in Settings, then relaunch.",
                if host.is_empty() { "(no URL set)" } else { host.as_str() }
            ));
            return;
        }
        // Point `jarvis serve` at the user's endpoint by writing the engine
        // host into ~/.openjarvis/config.toml (the env var alone is shadowed by
        // the engine's non-empty default host in the Python layer).
        if let Some((engine, host)) = &plan.engine_host {
            if let Err(e) = set_engine_host_in_config(engine, host) {
                let mut s = status.lock().await;
                s.error = Some(format!("Could not write engine config: {}", e));
                return;
            }
        }
        {
            let mut s = status.lock().await;
            s.ollama_ready = true;
            s.model_ready = true;
            s.detail = "Connected to custom endpoint.".into();
        }
    }

    // Phase 3: Start jarvis serve
    {
        let mut s = status.lock().await;
        s.phase = "server".into();
        s.detail = "Starting API server...".into();
    }

    let uv_bin = resolve_bin("uv");

    // Verify uv is actually installed. Concrete per-OS instructions —
    // the generic "install it from astral.sh" was the #1 source of
    // confusion on the Discord support thread; users couldn't tell whether
    // to use winget, scoop, pip, or the official installer.
    if !std::path::Path::new(&uv_bin).exists() && uv_bin == "uv" {
        let mut s = status.lock().await;
        #[cfg(target_os = "windows")]
        let msg = "Could not find 'uv' (Python package manager). \
                   To install on Windows, open PowerShell and run:\n\n\
                   powershell -ExecutionPolicy Bypass -c \"irm https://astral.sh/uv/install.ps1 | iex\"\n\n\
                   Then close and relaunch this app. \
                   (If the install completes but the app still can't find uv, \
                   you may need to log out and back in so PATH refreshes.)";
        #[cfg(target_os = "macos")]
        let msg = "Could not find 'uv' (Python package manager). \
                   To install on macOS, open Terminal and run:\n\n\
                   curl -LsSf https://astral.sh/uv/install.sh | sh\n\n\
                   Then relaunch this app.";
        #[cfg(target_os = "linux")]
        let msg = "Could not find 'uv' (Python package manager). \
                   To install on Linux, open a terminal and run:\n\n\
                   curl -LsSf https://astral.sh/uv/install.sh | sh\n\n\
                   Then relaunch this app.";
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        let msg = "Could not find 'uv' (Python package manager). \
                   Install it from https://astral.sh/uv then relaunch.";
        s.error = Some(msg.into());
        return;
    }

    let project_root = find_project_root();

    if project_root.is_none() {
        let mut s = status.lock().await;
        s.error = Some(
            "The verified backend resource bundled with this desktop app was not found. \
             Reinstall the matching package; do not point the app to an arbitrary local clone."
                .into(),
        );
        return;
    }

    // If something is already serving on our port, decide what to do based
    // on what it actually responds with — don't blindly kill it (#455).
    //
    // The OLD behaviour was: any HTTP response (even 404) → `fuser -k 8000/tcp`
    // / `taskkill /PID /F`. That broke the legitimate case where a user had
    // already started `jarvis serve` in a terminal and then launched the
    // desktop app — the app killed their server, then raced to spawn its
    // own, sometimes losing the race and hanging.
    //
    // New behaviour, by response shape:
    //   * 2xx /health        — healthy jarvis serve. Attach to it; skip the
    //                          uv-sync + spawn dance entirely. Done.
    //   * 503                — server is up but engine isn't ready. Surface
    //                          an actionable message; don't kill (matches
    //                          our wait_for_jarvis_health 503 contract).
    //   * any other status   — something else is listening on the port. Tell
    //                          the user via the error banner instead of
    //                          force-killing a foreign service.
    //   * Err (conn refused) — nothing is listening. Proceed to spawn.
    //
    // TODO(#455 follow-up): validate /health response body before attaching
    // so a multi-user host can't trivially spoof us. Also accept a port
    // override from config instead of hard-coding JARVIS_PORT.
    {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        match client
            .get(format!("http://127.0.0.1:{}/health", JARVIS_PORT))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                // Confirm with a second probe — the first might have caught
                // a flickering server (engine half-loaded, dying mid-stop,
                // etc.) and we don't want to claim ready off a 2-second
                // snapshot. Small sleep between to give the server room.
                tokio::time::sleep(Duration::from_millis(500)).await;
                let confirm = client
                    .get(format!("http://127.0.0.1:{}/health", JARVIS_PORT))
                    .send()
                    .await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false);
                if !confirm {
                    // First probe was 2xx but the second wasn't — fall
                    // through to the spawn path. The server probably went
                    // away between probes.
                    // (No early return — we want to spawn our own.)
                } else {
                    // Attach to the existing healthy server. Mark every
                    // pre-spawn step done so the setup UI doesn't show a
                    // half-progress bar (model_ready / ollama_ready stay
                    // false otherwise because we skipped those steps).
                    let mut s = status.lock().await;
                    s.phase = "ready".into();
                    s.detail = format!(
                        "Connected to existing API server on port {}.",
                        JARVIS_PORT,
                    );
                    s.server_ready = true;
                    s.model_ready = true;
                    s.ollama_ready = true;
                    return;
                }
            }
            Ok(resp) if resp.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE => {
                let mut s = status.lock().await;
                s.error = Some(format!(
                    "An API server is already running on port {} but its \
                     inference engine isn't ready (HTTP 503). If this is your \
                     `jarvis serve`, wait for it to finish loading and relaunch. \
                     Otherwise, stop that service or change the port.",
                    JARVIS_PORT,
                ));
                return;
            }
            Ok(resp) => {
                // Something else (a different web server, a stale process,
                // a 4xx-returning instance) is on our port. Don't kill it —
                // give the user actionable info instead.
                let mut s = status.lock().await;
                s.error = Some(format!(
                    "Port {} is already in use by another service (it answered \
                     /health with HTTP {}). Stop that service or change the \
                     OpenJarvis port, then relaunch.\n\nTo identify it:\n  {}",
                    JARVIS_PORT,
                    resp.status(),
                    port_owner_hint(),
                ));
                return;
            }
            Err(_) => {
                // Nothing listening — proceed to the normal spawn path.
            }
        }
    }

    if let Err(err) = check_jarvis_port_available() {
        let mut s = status.lock().await;
        s.error = Some(err);
        return;
    }

    let root = project_root.as_ref().unwrap();

    let cargo_bin = resolve_bin("cargo");
    if !std::path::Path::new(&cargo_bin).exists() && cargo_bin == "cargo" {
        let mut s = status.lock().await;
        s.error = Some(format_missing_rust_toolchain());
        return;
    }

    // Install dependencies automatically (handles fresh clones).
    //
    // Previously we ran `uv sync` with both stdout AND stderr piped to
    // /dev/null and discarded the exit code (`let _ = …`). When `uv sync`
    // failed — Windows path issues, network problems, lockfile conflicts —
    // the user saw no error, the boot continued, `uv run jarvis serve`
    // then ran in an under-provisioned venv, and the user waited the full
    // 600s health-check window before getting "Jarvis server did not
    // become healthy in time" with no actionable detail (issue #331).
    //
    // Now: capture stderr, check the exit status, surface a useful error
    // to the user BEFORE the long server-start wait. The status detail
    // message also indicates this can take a couple of minutes on first
    // boot so users don't restart the app thinking it's stuck.
    {
        let mut s = status.lock().await;
        s.detail = "Installing dependencies (uv sync — may take 1-2 min on first boot)...".into();
    }
    let mut sync_cmd = tokio::process::Command::new(&uv_bin);
    sync_cmd
        .args([
            "sync",
            "--extra", "server",
            "--extra", "inference-cloud",
            "--extra", "inference-google",
            // openjarvis_rust lives in a uv dependency group (not the published
            // `desktop` extra) so pip installs from PyPI don't require it (#584).
            "--group", "desktop-native",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .current_dir(root);
    // Avoid LD_LIBRARY_PATH leak when running inside an AppImage (#455).
    prepare_subprocess_for_appimage(&mut sync_cmd);
    add_cargo_bin_to_path(&mut sync_cmd);
    let sync_output = sync_cmd.output().await;
    match sync_output {
        Ok(out) if !out.status.success() => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let mut s = status.lock().await;
            s.error = Some(format_uv_sync_failure(root, out.status.code(), &stderr));
            return;
        }
        Err(e) => {
            let mut s = status.lock().await;
            s.error = Some(format_uv_sync_spawn_error(root, &uv_bin, &e.to_string()));
            return;
        }
        Ok(_) => {} // success — fall through
    }

    {
        let mut s = status.lock().await;
        s.detail = "Verifying Rust extension (openjarvis_rust)...".into();
    }
    if let Err(err) = verify_openjarvis_rust_extension(root, &uv_bin).await {
        let mut s = status.lock().await;
        s.error = Some(err);
        return;
    }

    {
        let mut s = status.lock().await;
        s.detail = format!("Starting API server from {}...", root.display());
    }

    let mut cmd = tokio::process::Command::new(&uv_bin);
    let mut serve_argv: Vec<String> = vec![
        "run".into(),
        "jarvis".into(),
        "serve".into(),
        "--port".into(),
        JARVIS_PORT.to_string(),
    ];
    serve_argv.extend(plan.serve_args.iter().cloned());
    // If the Ollama pull fell back to a different tag than planned, serve the
    // tag that is actually present. boot_plan always emits `--model` followed
    // immediately by its value, so `i + 1` is in bounds.
    if let Some(m) = &serve_model_override {
        match serve_argv.iter().position(|a| a == "--model") {
            Some(i) if i + 1 < serve_argv.len() => serve_argv[i + 1] = m.clone(),
            _ => eprintln!(
                "Warning: resolved model {:?} could not be applied; \
                 '--model <value>' not found in serve args {:?}",
                m, serve_argv
            ),
        }
    }
    cmd.args(&serve_argv)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .current_dir(root);
    // Avoid LD_LIBRARY_PATH leak when running inside an AppImage (#455) —
    // do this BEFORE cmd.env() calls below so our explicit cloud-key env
    // additions aren't accidentally stripped.
    prepare_subprocess_for_appimage(&mut cmd);

    // Enable only the bounded, approval-gated local actions. This is distinct
    // from OPENJARVIS_ENABLE_DANGEROUS_TOOLS, which remains unset.
    cmd.env("OPENJARVIS_ENABLE_CONTROLLED_LOCAL_ACTIONS", "1");
    // Every controlled app/document launch is checked locally before it can
    // execute. An unavailable or inconclusive Defender check denies the action.
    cmd.env("OPENJARVIS_ENABLE_EXECUTION_GUARD", "1");
    // The model can only propose structured UI plans. A separate native broker
    // validates the user-approved plan before any Windows UI Automation action.
    cmd.env("OPENJARVIS_ENABLE_CONTROLLED_DESKTOP_OPERATOR", "1");
    // Android diagnostics are a separate, read-only and approval-gated broker.
    // The backend receives no ADB binary path or Android serial number.
    cmd.env("OPENJARVIS_ENABLE_CONTROLLED_ANDROID_ADB", "1");

    #[cfg(target_os = "windows")]
    let desktop_broker_token = match desktop_broker::launch_token() {
        Ok(token) => token,
        Err(error) => {
            let mut s = status.lock().await;
            s.error = Some(error);
            return;
        }
    };
    #[cfg(target_os = "windows")]
    cmd.env("OPENJARVIS_DESKTOP_BROKER_TOKEN", &desktop_broker_token);

    #[cfg(target_os = "windows")]
    let android_adb_broker_token = match android_adb_broker::launch_token() {
        Ok(token) => token,
        Err(error) => {
            let mut s = status.lock().await;
            s.error = Some(error);
            return;
        }
    };
    #[cfg(target_os = "windows")]
    cmd.env("OPENJARVIS_ANDROID_ADB_BROKER_TOKEN", &android_adb_broker_token);

    // Do not inherit cloud credentials from the desktop process. The backend
    // receives only the explicitly selected inference credential below.
    for key_name in MANAGED_CLOUD_KEY_NAMES {
        cmd.env_remove(key_name);
    }
    cmd.env_remove("MINIMAX_API_KEY");
    cmd.env("OPENJARVIS_SECURE_DESKTOP_PROFILE", "1");

    // The backend receives only the explicitly selected provider identity and
    // its one credential. No inactive profile is exposed to the process.
    if let Some(provider) = cfg.provider.as_deref() {
        cmd.env("OPENJARVIS_CLOUD_PROVIDER", provider);
    }
    if let Some(endpoint) = cfg.provider_endpoint.as_deref() {
        cmd.env("OPENJARVIS_CLOUD_PROVIDER_ENDPOINT", endpoint);
    }
    // Inject only the selected cloud API key from secure desktop storage.
    for (key, value) in read_cloud_keys() {
        cmd.env(&key, &value);
    }
    let jarvis_child = cmd.spawn();

    match jarvis_child {
        Ok(mut child) => {
            // Start draining stderr immediately. If we wait until the
            // health check returns we risk filling the 4 KB Windows pipe
            // buffer during startup logging and hanging the child before
            // it can bind its HTTP port — exactly the symptom in #309.
            let stderr_handle = child.stderr.take();
            let mut mgr = backend.lock().await;
            let tail = mgr.jarvis_stderr_tail.clone();
            mgr.jarvis = Some(ChildHandle { child });
            drop(mgr);
            if let Some(stderr) = stderr_handle {
                spawn_jarvis_stderr_drainer(stderr, tail);
            }
        }
        Err(e) => {
            let mut s = status.lock().await;
            s.error = Some(format!(
                "Could not start jarvis server: {}. \
                 Make sure uv is installed (https://astral.sh/uv) and the OpenJarvis repo is cloned at {}",
                e,
                root.display(),
            ));
            return;
        }
    }

    let server_url = format!("http://127.0.0.1:{}/health", JARVIS_PORT);
    match wait_for_jarvis_health(&server_url, Duration::from_secs(600), &backend).await {
        JarvisStartResult::Ready => {}
        JarvisStartResult::ServiceUnavailable(body) => {
            let mut s = status.lock().await;
            s.error = Some(format!(
                "Jarvis server is running but the inference engine is not available \
                 (HTTP 503). This usually means the configured model couldn't be loaded.\n\n\
                 Check the server logs, or run 'uv run jarvis serve --port {}{}' \
                 from {} to see the engine error.\n\n\
                 Server response:\n{}",
                JARVIS_PORT,
                // Show the args actually passed (after `serve --port <port>`),
                // including any post-fallback `--model` override.
                match serve_argv.get(5..) {
                    Some(rest) if !rest.is_empty() => format!(" {}", rest.join(" ")),
                    _ => String::new(),
                },
                root.display(),
                body.trim(),
            ));
            return;
        }
        JarvisStartResult::EarlyExit { code, stderr } => {
            // `None` here means the OS didn't expose an exit code — on
            // Unix that's a signal kill (SIGKILL/SIGSEGV/...), on Windows
            // it means the process was terminated externally (Task
            // Manager, parent-of-parent, AV). "unknown" covers both.
            let code_str = code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "unknown".into());
            let mut s = status.lock().await;
            s.error = Some(if stderr.is_empty() {
                format!(
                    "Jarvis server exited (code {}) before becoming ready.\n\n\
                     No stderr output. Check that:\n\
                     1. uv is installed ({})\n\
                     2. The OpenJarvis repo is at {}\n\
                     3. 'uv sync' completes in that directory",
                    code_str,
                    uv_bin,
                    root.display(),
                )
            } else {
                format!(
                    "Jarvis server exited (code {}) before becoming ready.\n\nStderr:\n{}",
                    code_str, stderr,
                )
            });
            return;
        }
        JarvisStartResult::Timeout => {
            let stderr = read_jarvis_stderr_tail(&backend).await;
            let mut s = status.lock().await;
            s.error = Some(if stderr.is_empty() {
                format!(
                    "Jarvis server did not become ready within 10 minutes. Check that:\n\
                     1. uv is installed ({})\n\
                     2. The OpenJarvis repo is at {}\n\
                     3. Run 'uv sync' in that directory",
                    uv_bin,
                    root.display(),
                )
            } else {
                format!(
                    "Jarvis server did not become ready within 10 minutes.\n\nStderr:\n{}",
                    stderr,
                )
            });
            return;
        }
    }

    {
        let mut s = status.lock().await;
        s.server_ready = true;
        s.phase = "ready".into();
        s.detail = "All systems ready.".into();
    }

    #[cfg(target_os = "windows")]
    desktop_broker::spawn_worker(desktop_broker_token, api_base());
    #[cfg(target_os = "windows")]
    android_adb_broker::spawn_worker(android_adb_broker_token, api_base());

    // Phase 4: done. We intentionally do NOT auto-pull the rest of the
    // Qwen3.5 ladder here. The previous behavior walked every model that
    // "fit" in RAM (up to qwen3.5:122b ≈ 81 GB) and pulled each one in an
    // un-cancellable background task — so the app silently consumed tens of
    // gigabytes with no way to stop short of deleting it. The startup model
    // pulled in Phase 2 is enough to make the app fully usable; additional
    // models are now opt-in (Settings → "ollama pull <model>", or the
    // `pull_model` command invoked from the UI).
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

fn api_base() -> String {
    format!("http://127.0.0.1:{}", JARVIS_PORT)
}

#[tauri::command]
async fn get_setup_status(state: tauri::State<'_, SharedStatus>) -> Result<SetupStatus, String> {
    Ok(state.lock().await.clone())
}

#[tauri::command]
fn get_api_base() -> String {
    api_base()
}

#[tauri::command]
async fn start_backend(
    backend: tauri::State<'_, SharedBackend>,
    status: tauri::State<'_, SharedStatus>,
) -> Result<(), String> {
    let b = backend.inner().clone();
    let s = status.inner().clone();
    tauri::async_runtime::spawn(boot_backend(b, s));
    Ok(())
}

#[tauri::command]
async fn stop_backend(backend: tauri::State<'_, SharedBackend>) -> Result<(), String> {
    backend.lock().await.stop_all().await;
    Ok(())
}

#[tauri::command]
async fn check_health(api_url: String) -> Result<serde_json::Value, String> {
    let url = format!(
        "{}/health",
        if api_url.is_empty() {
            api_base()
        } else {
            api_url
        }
    );
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))
}

#[derive(serde::Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SecureSelfTestCheck {
    id: String,
    status: String,
    title: String,
    detail: String,
}

#[derive(serde::Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SecureSelfTestReport {
    checks: Vec<SecureSelfTestCheck>,
    passed: usize,
    warnings: usize,
    live_checks_required: usize,
}

fn secure_self_test_check(
    id: &str,
    status: &str,
    title: &str,
    detail: &str,
) -> SecureSelfTestCheck {
    SecureSelfTestCheck {
        id: id.to_string(),
        status: status.to_string(),
        title: title.to_string(),
        detail: detail.to_string(),
    }
}

fn secure_self_test_report(checks: Vec<SecureSelfTestCheck>) -> SecureSelfTestReport {
    let passed = checks.iter().filter(|check| check.status == "pass").count();
    let warnings = checks
        .iter()
        .filter(|check| matches!(check.status.as_str(), "warning" | "not_configured"))
        .count();
    let live_checks_required = checks
        .iter()
        .filter(|check| check.status == "live_check_required")
        .count();
    SecureSelfTestReport {
        checks,
        passed,
        warnings,
        live_checks_required,
    }
}

/// Report local-only. It deliberately does not start a broker, invoke ADB,
/// launch a browser, enumerate windows, or send cloud traffic.
#[tauri::command]
async fn run_secure_self_test() -> Result<SecureSelfTestReport, String> {
    let mut checks = Vec::new();
    let inference = read_inference_config();

    if inference.kind != SourceKind::Cloud {
        // A local (Ollama/custom) source is a fully supported configuration;
        // no warning is required.
        let label = match inference.kind {
            SourceKind::Ollama => "Modello locale (Ollama)",
            SourceKind::Custom => "Endpoint locale personalizzato",
            SourceKind::Cloud => unreachable!(),
        };
        checks.push(secure_self_test_check(
            "local-inference",
            "pass",
            label,
            "L'inferenza locale è selezionata; nessun provider cloud è richiesto.",
        ));
    } else if let Some(provider) = inference.provider.as_deref() {
        let key_present = cloud_api_key_name(provider)
            .and_then(|key_name| secure_store_get(key_name).ok().flatten())
            .is_some_and(|value| !value.is_empty());
        if !inference.provider_processing_acknowledged {
            checks.push(secure_self_test_check(
                "cloud-provider-consent",
                "warning",
                "Consenso provider cloud",
                "Conferma nelle Impostazioni che il provider selezionato riceve e processa prompt e risposte tramite TLS.",
            ));
        } else if key_present {
            checks.push(secure_self_test_check(
                "cloud-provider",
                "pass",
                "Provider cloud",
                "Il provider selezionato ha una credenziale nel portachiavi del sistema operativo.",
            ));
        } else {
            checks.push(secure_self_test_check(
                "cloud-provider-key",
                "warning",
                "Credenziale provider cloud",
                "Manca la credenziale del provider selezionato nel portachiavi del sistema operativo.",
            ));
        }
    } else {
        checks.push(secure_self_test_check(
            "cloud-provider",
            "not_configured",
            "Provider cloud",
            "Nessun provider cloud selezionato. Il backend locale resta comunque utilizzabile; il cloud è opzionale.",
        ));
    }

    let backend_url = format!("{}/health", api_base());
    let backend_healthy = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|_| "Unable to create the local health client.")?
        .get(&backend_url)
        .send()
        .await
        .is_ok_and(|response| response.status().is_success());
    checks.push(if backend_healthy {
        secure_self_test_check(
            "backend-health",
            "pass",
            "Backend locale",
            "Il backend locale risponde al controllo health.",
        )
    } else {
        secure_self_test_check(
            "backend-health",
            "warning",
            "Backend locale",
            "Il backend locale non risponde ancora. Attendi l’avvio o riavvia l’app.",
        )
    });

    let transcription = read_transcription_config();
    let transcription_enabled = transcription.provider.as_deref() == Some(GROQ_TRANSCRIPTION_PROVIDER)
        && transcription.processing_acknowledged;
    let groq_key_present = matches!(secure_store_get("GROQ_API_KEY"), Ok(Some(value)) if !value.is_empty());
    checks.push(if transcription_enabled && groq_key_present {
        secure_self_test_check(
            "voice-transcription",
            "pass",
            "Trascrizione vocale",
            "Groq Whisper è configurato; il test non registra né invia audio.",
        )
    } else {
        secure_self_test_check(
            "voice-transcription",
            "not_configured",
            "Trascrizione vocale",
            "Groq Whisper non è configurato completamente. Il test non registra né invia audio.",
        )
    });

    let adb = read_android_adb_config();
    let adb_ready = adb.diagnostics_acknowledged
        && adb.adb_path.as_deref().is_some_and(|path| std::path::Path::new(path).is_file())
        && adb.device_serial.as_deref().is_some_and(is_safe_android_adb_serial);
    checks.push(if adb_ready {
        secure_self_test_check(
            "android-adb",
            "live_check_required",
            "Diagnostica Android ADB",
            "Configurazione rilevata. Collega e autorizza un Android reale per la verifica manuale; questo test non invia comandi ADB.",
        )
    } else {
        secure_self_test_check(
            "android-adb",
            "not_configured",
            "Diagnostica Android ADB",
            "Non configurata. Il test non cerca dispositivi e non avvia ADB.",
        )
    });

    checks.push(secure_self_test_check(
        "browser-policy",
        "pass",
        "Browser controllato",
        "Nel profilo desktop sicuro, lettura HTTPS e ricerca sono consentite; login, credenziali, pagamenti, invii, eliminazioni e URL sensibili sono bloccati localmente.",
    ));
    checks.push(secure_self_test_check(
        "windows-ui-automation",
        "live_check_required",
        "Broker Windows UI Automation",
        "Richiede build e prova manuale su Windows con una finestra non sensibile; questo test non controlla finestre né inserisce testo.",
    ));
    let gemini_live_enabled = read_gemini_live_config().processing_acknowledged;
    let gemini_key_present = matches!(secure_store_get("GEMINI_API_KEY"), Ok(Some(value)) if !value.is_empty());
    checks.push(if gemini_live_enabled && gemini_key_present {
        secure_self_test_check(
            "gemini-live",
            "live_check_required",
            "Gemini Live",
            "Configurazione pronta. Richiede prova manuale con microfono, rete e account Google; il test non crea token né invia audio.",
        )
    } else {
        secure_self_test_check(
            "gemini-live",
            "not_configured",
            "Gemini Live",
            "Non configurato. Salva una chiave Gemini e il consenso separato nelle Impostazioni prima di una prova manuale.",
        )
    });

    Ok(secure_self_test_report(checks))
}

#[tauri::command]
async fn fetch_energy(api_url: String) -> Result<serde_json::Value, String> {
    let base = if api_url.is_empty() {
        api_base()
    } else {
        api_url
    };
    let resp = reqwest::get(format!("{}/v1/telemetry/energy", base))
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))
}

#[tauri::command]
async fn fetch_telemetry(api_url: String) -> Result<serde_json::Value, String> {
    let base = if api_url.is_empty() {
        api_base()
    } else {
        api_url
    };
    let resp = reqwest::get(format!("{}/v1/telemetry/stats", base))
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))
}

#[tauri::command]
async fn fetch_traces(api_url: String, limit: u32) -> Result<serde_json::Value, String> {
    let base = if api_url.is_empty() {
        api_base()
    } else {
        api_url
    };
    let resp = reqwest::get(format!("{}/v1/traces?limit={}", base, limit))
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))
}

#[tauri::command]
async fn fetch_trace(api_url: String, trace_id: String) -> Result<serde_json::Value, String> {
    let base = if api_url.is_empty() {
        api_base()
    } else {
        api_url
    };
    let resp = reqwest::get(format!("{}/v1/traces/{}", base, trace_id))
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))
}

#[tauri::command]
async fn fetch_learning_stats(api_url: String) -> Result<serde_json::Value, String> {
    let base = if api_url.is_empty() {
        api_base()
    } else {
        api_url
    };
    let resp = reqwest::get(format!("{}/v1/learning/stats", base))
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))
}

#[tauri::command]
async fn fetch_learning_policy(api_url: String) -> Result<serde_json::Value, String> {
    let base = if api_url.is_empty() {
        api_base()
    } else {
        api_url
    };
    let resp = reqwest::get(format!("{}/v1/learning/policy", base))
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))
}

#[tauri::command]
async fn fetch_memory_stats(api_url: String) -> Result<serde_json::Value, String> {
    let base = if api_url.is_empty() {
        api_base()
    } else {
        api_url
    };
    let resp = reqwest::get(format!("{}/v1/memory/stats", base))
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))
}

#[tauri::command]
async fn search_memory(
    api_url: String,
    query: String,
    top_k: u32,
) -> Result<serde_json::Value, String> {
    let base = if api_url.is_empty() {
        api_base()
    } else {
        api_url
    };
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/v1/memory/search", base))
        .json(&serde_json::json!({"query": query, "top_k": top_k}))
        .send()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))
}

#[tauri::command]
async fn fetch_agents(api_url: String) -> Result<serde_json::Value, String> {
    let base = if api_url.is_empty() {
        api_base()
    } else {
        api_url
    };
    let resp = reqwest::get(format!("{}/v1/agents", base))
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))
}

#[tauri::command]
async fn fetch_models(api_url: String) -> Result<serde_json::Value, String> {
    let base = if api_url.is_empty() {
        api_base()
    } else {
        api_url
    };
    let resp = reqwest::get(format!("{}/v1/models", base))
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))
}

#[tauri::command]
async fn fetch_savings(api_url: String) -> Result<serde_json::Value, String> {
    let base = if api_url.is_empty() {
        api_base()
    } else {
        api_url
    };
    let resp = reqwest::get(format!("{}/v1/savings", base))
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    resp.json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))
}

/// Transcribe a user-recorded file through the explicitly enabled Groq
/// provider. The renderer never receives the credential and the Python chat
/// backend is not given this separate speech credential.
#[tauri::command]
async fn transcribe_audio(
    _api_url: String,
    audio_data: Vec<u8>,
    filename: String,
) -> Result<serde_json::Value, String> {
    let cfg = read_transcription_config();
    if cfg.provider.as_deref() != Some(GROQ_TRANSCRIPTION_PROVIDER)
        || !cfg.processing_acknowledged
    {
        return Err("Groq Whisper transcription is not enabled in Settings.".into());
    }
    if audio_data.is_empty() {
        return Err("The recording is empty.".into());
    }
    if audio_data.len() > MAX_TRANSCRIPTION_AUDIO_BYTES {
        return Err("The recording exceeds the 25 MB transcription upload limit.".into());
    }
    let extension = filename
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mime_type = match extension.as_str() {
        "webm" => "audio/webm",
        "wav" => "audio/wav",
        "mp3" | "mpga" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "mp4" => "video/mp4",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        _ => return Err("This recording format is not supported by Groq Whisper.".into()),
    };
    let api_key = secure_store_get("GROQ_API_KEY")?
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Save a Groq API key in Settings before transcribing audio.".to_string())?;

    let part = reqwest::multipart::Part::bytes(audio_data)
        .file_name(format!("recording.{}", extension))
        .mime_str(mime_type)
        .map_err(|_| "Unable to prepare the recording for transcription.")?;
    let form = reqwest::multipart::Form::new()
        .text("model", cfg.model)
        .text("response_format", "verbose_json")
        .part("file", part);
    let response = reqwest::Client::new()
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|_| "Unable to contact the selected transcription provider.")?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("The transcription provider rejected the recording (HTTP {}).", status.as_u16()));
    }
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|_| "The transcription provider returned an invalid response.")?;
    Ok(serde_json::json!({
        "text": body.get("text").and_then(|value| value.as_str()).unwrap_or_default(),
        "language": body.get("language").and_then(|value| value.as_str()),
        "confidence": serde_json::Value::Null,
        "duration_seconds": body.get("duration").and_then(|value| value.as_f64()).unwrap_or(0.0),
    }))
}

/// Submit savings to Supabase leaderboard.
#[tauri::command]
async fn submit_savings(
    supabase_url: String,
    supabase_key: String,
    payload: serde_json::Value,
) -> Result<bool, String> {
    if supabase_url.is_empty() || supabase_key.is_empty() {
        return Ok(false);
    }
    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "{}/rest/v1/savings_entries?on_conflict=anon_id",
            supabase_url
        ))
        .header("Content-Type", "application/json")
        .header("apikey", &supabase_key)
        .header("Authorization", format!("Bearer {}", supabase_key))
        .header("Prefer", "resolution=merge-duplicates")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Supabase POST failed: {}", e))?;
    Ok(resp.status().is_success())
}

// ---------------------------------------------------------------------------
// Cloud API key management
// ---------------------------------------------------------------------------

const SECURE_KEY_SERVICE: &str = "OpenJarvis Cloud Keys";
const MANAGED_CLOUD_KEY_NAMES: &[&str] = &[
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "OPENROUTER_API_KEY",
    "GROQ_API_KEY",
    "NVIDIA_API_KEY",
    "SAMBANOVA_API_KEY",
    "DASHSCOPE_API_KEY",
    "POLLINATIONS_API_KEY",
    "HF_TOKEN",
    "TOGETHER_API_KEY",
    "PICOVOICE_ACCESS_KEY",
    "TAVILY_API_KEY",
];

/// Legacy path used by older desktop builds. New saves never write here.
fn legacy_cloud_keys_path() -> std::path::PathBuf {
    let home = home_dir();
    std::path::PathBuf::from(home)
        .join(".openjarvis")
        .join("cloud-keys.env")
}

fn validate_cloud_key_name(key_name: &str) -> Result<(), String> {
    let valid = !key_name.is_empty()
        && key_name.len() <= 128
        && (key_name.ends_with("_API_KEY")
            || matches!(key_name, "HF_TOKEN" | "PICOVOICE_ACCESS_KEY"))
        && key_name
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_');
    if valid {
        Ok(())
    } else {
        Err(format!("Invalid API key name: {}", key_name))
    }
}

fn engine_api_key_name(engine: &str) -> String {
    let normalized: String = engine
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = normalized.trim_matches('_');
    let engine_name = if trimmed.is_empty() {
        CUSTOM_FALLBACK_ENGINE.to_ascii_uppercase()
    } else {
        trimmed.to_string()
    };
    format!("{}_API_KEY", engine_name)
}

fn managed_cloud_key_names() -> Vec<String> {
    let mut names: Vec<String> = MANAGED_CLOUD_KEY_NAMES
        .iter()
        .map(|name| (*name).to_string())
        .collect();

    let cfg = read_inference_config();
    if matches!(&cfg.kind, SourceKind::Custom) {
        let engine = cfg.engine.unwrap_or_else(|| CUSTOM_FALLBACK_ENGINE.to_string());
        let key_name = engine_api_key_name(&engine);
        if validate_cloud_key_name(&key_name).is_ok() {
            names.push(key_name);
        }
    }

    names.sort();
    names.dedup();
    names
}

fn secure_store_get(key_name: &str) -> Result<Option<String>, String> {
    validate_cloud_key_name(key_name)?;
    let entry = keyring::Entry::new(SECURE_KEY_SERVICE, key_name)
        .map_err(|err| format!("Failed to open secure key storage for {}: {}", key_name, err))?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(format!("Failed to read {} from secure key storage: {}", key_name, err)),
    }
}

fn secure_store_set(key_name: &str, key_value: &str) -> Result<(), String> {
    validate_cloud_key_name(key_name)?;
    let entry = keyring::Entry::new(SECURE_KEY_SERVICE, key_name)
        .map_err(|err| format!("Failed to open secure key storage for {}: {}", key_name, err))?;
    if key_value.is_empty() {
        return match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(format!(
                "Failed to remove {} from secure key storage: {}",
                key_name, err
            )),
        };
    }
    entry
        .set_password(key_value)
        .map_err(|err| format!("Failed to save {} in secure key storage: {}", key_name, err))
}

fn read_legacy_cloud_keys() -> Vec<(String, String)> {
    let path = legacy_cloud_keys_path();
    let mut keys = Vec::new();
    if let Ok(contents) = std::fs::read_to_string(&path) {
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                keys.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    keys
}

fn migrate_legacy_cloud_keys() {
    let path = legacy_cloud_keys_path();
    if !path.exists() {
        return;
    }

    let legacy_keys = read_legacy_cloud_keys();
    if legacy_keys.is_empty() {
        let _ = std::fs::remove_file(&path);
        return;
    }

    let mut migrated_all = true;
    for (key, value) in legacy_keys {
        if value.is_empty() {
            continue;
        }
        if secure_store_set(&key, &value).is_err() {
            migrated_all = false;
        }
    }

    if migrated_all {
        let _ = std::fs::remove_file(path);
    }
}

/// Read only the key explicitly authorized for the active cloud profile.
fn read_cloud_keys() -> Vec<(String, String)> {
    migrate_legacy_cloud_keys();
    let config = read_inference_config();
    if !config.provider_processing_acknowledged {
        return Vec::new();
    }
    let provider = config.provider.unwrap_or_default();
    let Some(key_name) = cloud_api_key_name(&provider) else {
        return Vec::new();
    };
    match secure_store_get(key_name) {
        Ok(Some(value)) if !value.is_empty() => vec![(key_name.to_string(), value)],
        _ => Vec::new(),
    }
}

/// Save a single cloud API key to secure desktop storage.
///
/// The key takes effect after a restart. This avoids sending a newly saved
/// credential to an already-running backend that was started for another
/// profile, and therefore preserves single-profile key isolation.
#[tauri::command]
async fn save_cloud_key(key_name: String, key_value: String) -> Result<(), String> {
    let key_value = key_value.trim().to_string();
    secure_store_set(&key_name, &key_value)
}

/// Get which cloud providers have keys configured (without exposing values).
#[tauri::command]
async fn get_cloud_key_status() -> Result<serde_json::Value, String> {
    migrate_legacy_cloud_keys();
    let status: Vec<serde_json::Value> = managed_cloud_key_names()
        .into_iter()
        .map(|key| {
            let set = matches!(secure_store_get(&key), Ok(Some(value)) if !value.is_empty());
            serde_json::json!({ "key": key, "set": set })
        })
        .collect();
    Ok(serde_json::json!(status))
}

/// Return the current inference-source config for the Settings UI.
#[tauri::command]
async fn get_inference_source() -> Result<InferenceConfig, String> {
    Ok(read_inference_config())
}

/// Persist the chosen inference source. `host` is normalized to a bare base
/// URL. For custom endpoints, an optional API key is stored in secure desktop
/// storage under `<ENGINE>_API_KEY`. Applies on next app launch.
#[tauri::command]
async fn set_inference_source(
    kind: String,
    model: Option<String>,
    host: Option<String>,
    engine: Option<String>,
    provider: Option<String>,
    api_key: Option<String>,
    provider_endpoint: Option<String>,
    provider_processing_acknowledged: bool,
) -> Result<(), String> {
    let kind = kind.trim().to_ascii_lowercase();
    match kind.as_str() {
        "ollama" => {
            let model = model.unwrap_or_default().trim().to_string();
            let cfg = InferenceConfig {
                kind: SourceKind::Ollama,
                model: Some(if model.is_empty() {
                    default_local_model(total_ram_gb()).to_string()
                } else {
                    model
                }),
                host: None,
                engine: None,
                provider: None,
                provider_endpoint: None,
                provider_processing_acknowledged: false,
            };
            return write_inference_config(&cfg);
        }
        "custom" => {
            let host = host.unwrap_or_default().trim().to_string();
            let engine = engine
                .unwrap_or_else(|| CUSTOM_FALLBACK_ENGINE.to_string())
                .trim()
                .to_string();
            let model = model.unwrap_or_default().trim().to_string();
            let cfg = InferenceConfig {
                kind: SourceKind::Custom,
                model: Some(model),
                host: Some(host),
                engine: Some(engine),
                provider: None,
                provider_endpoint: None,
                provider_processing_acknowledged: false,
            };
            return write_inference_config(&cfg);
        }
        "cloud" => {}
        _ => {
            return Err(
                "Unsupported inference source. Choose cloud, ollama, or a custom local endpoint."
                    .into(),
            );
        }
    }

    // Cloud path below — unchanged.
    if !provider_processing_acknowledged {
        return Err(
            "Confirm that TLS protects data in transit while the selected provider processes prompts and responses before saving a cloud profile."
                .into(),
        );
    }
    let provider = provider.unwrap_or_default().trim().to_ascii_lowercase();
    validate_cloud_provider(&provider)?;
    let provider_endpoint = validate_provider_endpoint(&provider, provider_endpoint.as_deref())?;
    let model = model.unwrap_or_default().trim().to_string();
    if model.is_empty() {
        return Err("A cloud model is required.".into());
    }
    let key_name = cloud_api_key_name(&provider)
        .ok_or_else(|| "The selected provider has no supported credential mapping.".to_string())?;
    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        save_cloud_key(key_name.to_string(), key)
            .await
            .map_err(|e| format!("Could not store the API key: {}", e))?;
    } else if !matches!(secure_store_get(key_name), Ok(Some(value)) if !value.is_empty()) {
        return Err("An API key for the selected provider is required and is stored only in the operating-system credential store.".into());
    }
    let cfg = InferenceConfig {
        kind: SourceKind::Cloud,
        model: Some(model),
        host: None,
        engine: None,
        provider: Some(provider),
        provider_endpoint,
        provider_processing_acknowledged: true,
    };
    write_inference_config(&cfg)
}

// ---------------------------------------------------------------------------
// Inference-source selection (~/.openjarvis/inference.json)
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum SourceKind {
    Ollama,
    Custom,
    Cloud,
}

impl Default for SourceKind {
    fn default() -> Self {
        SourceKind::Cloud
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
struct InferenceConfig {
    #[serde(default)]
    kind: SourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    /// Bare base URL (no trailing `/v1`), custom only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    /// OpenAI-compatible engine key (e.g. "lmstudio"), custom only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    engine: Option<String>,
    /// Explicit cloud provider allowlisted for this desktop profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    /// Optional approved provider endpoint for regional/provider-console routes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_endpoint: Option<String>,
    /// User acknowledgement that a normal cloud provider processes plaintext.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    provider_processing_acknowledged: bool,
}

/// Path to the inference-source config (~/.openjarvis/inference.json).
fn inference_config_path() -> std::path::PathBuf {
    std::path::PathBuf::from(home_dir())
        .join(".openjarvis")
        .join("inference.json")
}

const GROQ_TRANSCRIPTION_PROVIDER: &str = "groq-whisper";
const GROQ_TRANSCRIPTION_MODELS: &[&str] = &["whisper-large-v3-turbo", "whisper-large-v3"];
const MAX_TRANSCRIPTION_AUDIO_BYTES: usize = 25 * 1024 * 1024;

/// Transcription settings never contain API keys; the Groq credential is read
/// only by the native command at the moment an approved recording is sent.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
struct TranscriptionConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    #[serde(default = "default_groq_transcription_model")]
    model: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    processing_acknowledged: bool,
}

fn default_groq_transcription_model() -> String {
    "whisper-large-v3-turbo".to_string()
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            provider: None,
            model: default_groq_transcription_model(),
            processing_acknowledged: false,
        }
    }
}

fn transcription_config_path() -> std::path::PathBuf {
    std::path::PathBuf::from(home_dir())
        .join(".openjarvis")
        .join("transcription.json")
}

fn read_transcription_config() -> TranscriptionConfig {
    std::fs::read_to_string(transcription_config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<TranscriptionConfig>(&text).ok())
        .filter(|cfg| {
            cfg.provider.as_deref() == Some(GROQ_TRANSCRIPTION_PROVIDER)
                && GROQ_TRANSCRIPTION_MODELS.contains(&cfg.model.as_str())
        })
        .unwrap_or_default()
}

fn write_transcription_config(cfg: &TranscriptionConfig) -> Result<(), String> {
    let path = transcription_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|_| "Unable to create the local settings directory.")?;
    }
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|_| "Unable to encode transcription settings.")?;
    std::fs::write(path, json + "\n")
        .map_err(|_| "Unable to save transcription settings.".to_string())
}

#[tauri::command]
async fn get_transcription_source() -> Result<TranscriptionConfig, String> {
    Ok(read_transcription_config())
}

#[tauri::command]
async fn set_transcription_source(
    provider: Option<String>,
    model: Option<String>,
    processing_acknowledged: bool,
) -> Result<(), String> {
    let provider = provider.unwrap_or_default().trim().to_ascii_lowercase();
    if provider.is_empty() {
        return write_transcription_config(&TranscriptionConfig::default());
    }
    if provider != GROQ_TRANSCRIPTION_PROVIDER {
        return Err("Only Groq Whisper is available as a cloud transcription provider in this desktop profile.".into());
    }
    if !processing_acknowledged {
        return Err("Confirm that the selected transcription provider receives the recorded audio over TLS and processes it before enabling transcription.".into());
    }
    let model = model.unwrap_or_else(default_groq_transcription_model);
    if !GROQ_TRANSCRIPTION_MODELS.contains(&model.as_str()) {
        return Err("The selected Groq transcription model is not supported by this desktop profile.".into());
    }
    if !matches!(secure_store_get("GROQ_API_KEY"), Ok(Some(value)) if !value.is_empty()) {
        return Err("Save a Groq API key in the operating-system credential store before enabling transcription.".into());
    }
    write_transcription_config(&TranscriptionConfig {
        provider: Some(provider),
        model,
        processing_acknowledged: true,
    })
}

#[tauri::command]
async fn get_transcription_status() -> Result<serde_json::Value, String> {
    let cfg = read_transcription_config();
    let enabled = cfg.provider.as_deref() == Some(GROQ_TRANSCRIPTION_PROVIDER)
        && cfg.processing_acknowledged;
    let key_set = matches!(secure_store_get("GROQ_API_KEY"), Ok(Some(value)) if !value.is_empty());
    let available = enabled && key_set;
    let reason = if available {
        None
    } else if !enabled {
        Some("Enable Groq Whisper and confirm audio processing in Settings.".to_string())
    } else {
        Some("Save a Groq API key in Settings.".to_string())
    };
    Ok(serde_json::json!({
        "available": available,
        "backend": if available { Some(GROQ_TRANSCRIPTION_PROVIDER) } else { None::<&str> },
        "reason": reason,
        "model": cfg.model,
    }))
}

const GEMINI_LIVE_MODEL: &str = "gemini-3.1-flash-live-preview";
const GEMINI_LIVE_TOKEN_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/auth_tokens";

/// Live settings never contain the Gemini key. The temporary session token is
/// minted only on an explicit renderer request and is not persisted by native
/// code or forwarded to the Python backend.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct GeminiLiveConfig {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    processing_acknowledged: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct GeminiLiveSessionToken {
    access_token: String,
    expires_at: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiLiveTokenResponse {
    name: String,
    expire_time: Option<String>,
}

fn gemini_live_config_path() -> std::path::PathBuf {
    std::path::PathBuf::from(home_dir())
        .join(".openjarvis")
        .join("gemini-live.json")
}

fn read_gemini_live_config() -> GeminiLiveConfig {
    std::fs::read_to_string(gemini_live_config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<GeminiLiveConfig>(&text).ok())
        .unwrap_or_default()
}

fn write_gemini_live_config(config: &GeminiLiveConfig) -> Result<(), String> {
    let path = gemini_live_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|_| "Unable to create the local settings directory.")?;
    }
    let content = serde_json::to_string_pretty(config)
        .map_err(|_| "Unable to encode Gemini Live settings.")?;
    std::fs::write(path, content + "\n")
        .map_err(|_| "Unable to save Gemini Live settings.".to_string())
}

#[tauri::command]
async fn get_gemini_live_config() -> Result<GeminiLiveConfig, String> {
    Ok(read_gemini_live_config())
}

#[tauri::command]
async fn set_gemini_live_config(processing_acknowledged: bool) -> Result<(), String> {
    if !processing_acknowledged {
        return write_gemini_live_config(&GeminiLiveConfig::default());
    }
    if !matches!(secure_store_get("GEMINI_API_KEY"), Ok(Some(value)) if !value.is_empty()) {
        return Err(
            "Save a Gemini API key in the operating-system credential store before enabling Gemini Live."
                .into(),
        );
    }
    write_gemini_live_config(&GeminiLiveConfig {
        processing_acknowledged: true,
    })
}

/// Mints a one-use, model-constrained token for a direct renderer-to-Gemini
/// Live WebSocket. The long-lived key stays in the OS keyring and the short
/// token is deliberately not logged, persisted, or passed to Python.
fn gemini_live_token_payload() -> serde_json::Value {
    serde_json::json!({
        "uses": 1,
        "liveConnectConstraints": {
            "model": format!("models/{GEMINI_LIVE_MODEL}"),
            "config": {
                "responseModalities": ["AUDIO"],
                "sessionResumption": {}
            }
        }
    })
}

#[tauri::command]
async fn mint_gemini_live_session_token() -> Result<GeminiLiveSessionToken, String> {
    if !read_gemini_live_config().processing_acknowledged {
        return Err(
            "Enable Gemini Live and confirm that Google processes streamed microphone audio before connecting."
                .into(),
        );
    }
    let api_key = secure_store_get("GEMINI_API_KEY")?
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Save a Gemini API key in the operating-system credential store.".to_string())?;
    let payload = gemini_live_token_payload();
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| "Unable to create the Gemini Live client.")?
        .post(GEMINI_LIVE_TOKEN_URL)
        .header("x-goog-api-key", api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|_| "Gemini Live token provisioning failed. Check the network and API access.")?;
    if !response.status().is_success() {
        return Err(format!(
            "Gemini Live token provisioning was rejected (HTTP {}).",
            response.status().as_u16()
        ));
    }
    let token = response
        .json::<GeminiLiveTokenResponse>()
        .await
        .map_err(|_| "Gemini Live returned an invalid token response.")?;
    if token.name.trim().is_empty() {
        return Err("Gemini Live returned an empty temporary token.".into());
    }
    Ok(GeminiLiveSessionToken {
        access_token: token.name,
        expires_at: token.expire_time,
    })
}

const ANDROID_ADB_CONFIG_FILE: &str = "android-adb.json";

/// Local Android ADB settings. Device serial and executable path stay native:
/// they are never copied into the cloud backend or approval payload.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
struct AndroidAdbConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    adb_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    device_serial: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    diagnostics_acknowledged: bool,
}

fn android_adb_config_path() -> std::path::PathBuf {
    std::path::PathBuf::from(home_dir())
        .join(".openjarvis")
        .join(ANDROID_ADB_CONFIG_FILE)
}

fn is_safe_android_adb_path(path: &str) -> bool {
    let normalized = path.trim().replace('/', "\\").to_ascii_lowercase();
    let bytes = normalized.as_bytes();
    bytes.len() > 26
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes[2] == b'\\'
        && normalized.ends_with("\\platform-tools\\adb.exe")
        && !normalized.contains("..")
}

fn is_safe_android_adb_serial(serial: &str) -> bool {
    !serial.is_empty()
        && serial.len() <= 128
        && serial
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':'))
}

fn read_android_adb_config() -> AndroidAdbConfig {
    std::fs::read_to_string(android_adb_config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<AndroidAdbConfig>(&text).ok())
        .filter(|config| {
            config
                .adb_path
                .as_deref()
                .is_some_and(is_safe_android_adb_path)
                && config
                    .device_serial
                    .as_deref()
                    .is_some_and(is_safe_android_adb_serial)
        })
        .unwrap_or_default()
}

fn write_android_adb_config(config: &AndroidAdbConfig) -> Result<(), String> {
    let path = android_adb_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|_| "Unable to create the local settings directory.")?;
    }
    let content = serde_json::to_string_pretty(config)
        .map_err(|_| "Unable to encode Android ADB settings.")?;
    std::fs::write(path, content + "\n")
        .map_err(|_| "Unable to save Android ADB settings.".to_string())
}

#[tauri::command]
async fn get_android_adb_config() -> Result<AndroidAdbConfig, String> {
    Ok(read_android_adb_config())
}

#[tauri::command]
async fn set_android_adb_config(
    adb_path: Option<String>,
    device_serial: Option<String>,
    diagnostics_acknowledged: bool,
) -> Result<(), String> {
    let adb_path = adb_path.unwrap_or_default().trim().to_string();
    let device_serial = device_serial.unwrap_or_default().trim().to_string();
    if adb_path.is_empty() && device_serial.is_empty() {
        return write_android_adb_config(&AndroidAdbConfig::default());
    }
    if !diagnostics_acknowledged {
        return Err("Confirm the read-only Android ADB diagnostic boundary before enabling it.".into());
    }
    if !is_safe_android_adb_path(&adb_path) || !std::path::Path::new(&adb_path).is_file() {
        return Err("Select the existing adb.exe inside an official Android SDK Platform Tools directory.".into());
    }
    if !is_safe_android_adb_serial(&device_serial) {
        return Err("Select one Android device reported by the local ADB discovery command.".into());
    }
    write_android_adb_config(&AndroidAdbConfig {
        adb_path: Some(adb_path),
        device_serial: Some(device_serial),
        diagnostics_acknowledged: true,
    })
}

#[derive(serde::Serialize)]
struct AndroidAdbDevice {
    serial: String,
    state: String,
    model: Option<String>,
}

#[tauri::command]
async fn discover_android_adb_devices(adb_path: String) -> Result<Vec<AndroidAdbDevice>, String> {
    let adb_path = adb_path.trim();
    if !is_safe_android_adb_path(adb_path) || !std::path::Path::new(adb_path).is_file() {
        return Err("Select the existing adb.exe inside an official Android SDK Platform Tools directory.".into());
    }
    let mut command = tokio::process::Command::new(adb_path);
    command.args(["devices", "-l"]);
    command.kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(8), command.output())
        .await
        .map_err(|_| "ADB discovery timed out.")?
        .map_err(|_| "Android Platform Tools could not run device discovery.")?;
    if !output.status.success() || output.stdout.len() > 64 * 1024 {
        return Err("ADB device discovery was rejected or produced unsafe output.".into());
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| "ADB device discovery returned invalid text.")?;
    let mut devices = Vec::new();
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 2 || !is_safe_android_adb_serial(fields[0]) {
            continue;
        }
        let model = fields
            .iter()
            .find_map(|field| field.strip_prefix("model:"))
            .filter(|model| model.len() <= 80 && model.chars().all(|ch| ch.is_ascii_graphic() || ch == '_'))
            .map(str::to_string);
        devices.push(AndroidAdbDevice {
            serial: fields[0].to_string(),
            state: fields[1].to_string(),
            model,
        });
    }
    Ok(devices)
}

const CONTROLLED_FOLDERS_FILE: &str = "controlled-local-folders.json";
const MAX_CONTROLLED_FOLDERS: usize = 8;

fn controlled_folders_path() -> std::path::PathBuf {
    std::path::PathBuf::from(home_dir())
        .join(".openjarvis")
        .join(CONTROLLED_FOLDERS_FILE)
}

fn is_safe_controlled_folder(folder: &std::path::Path) -> bool {
    let Ok(canonical) = folder.canonicalize() else {
        return false;
    };
    if !canonical.is_dir() || canonical.parent().is_none() {
        return false;
    }
    let home = std::path::PathBuf::from(home_dir());
    if canonical == home || canonical == home.join(".openjarvis") {
        return false;
    }
    let blocked = [
        ".aws",
        ".gnupg",
        ".openjarvis",
        ".ssh",
        "appdata",
        "program files",
        "program files (x86)",
        "system32",
        "windows",
    ];
    !canonical.components().any(|component| {
        let value = component.as_os_str().to_string_lossy().to_ascii_lowercase();
        blocked.iter().any(|name| value == *name)
    })
}

fn read_controlled_folders() -> Vec<String> {
    let Ok(value) = std::fs::read_to_string(controlled_folders_path()) else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&value) else {
        return Vec::new();
    };
    let Some(folders) = parsed.get("folders").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut safe = Vec::new();
    for raw in folders.iter().take(MAX_CONTROLLED_FOLDERS) {
        let Some(raw) = raw.as_str() else { continue };
        let path = std::path::PathBuf::from(raw);
        if path.is_absolute() && is_safe_controlled_folder(&path) {
            if let Ok(canonical) = path.canonicalize() {
                let value = canonical.to_string_lossy().to_string();
                if !safe.contains(&value) {
                    safe.push(value);
                }
            }
        }
    }
    safe
}

#[tauri::command]
async fn get_controlled_folders() -> Result<Vec<String>, String> {
    Ok(read_controlled_folders())
}

#[tauri::command]
async fn set_controlled_folders(folders: Vec<String>) -> Result<Vec<String>, String> {
    if folders.len() > MAX_CONTROLLED_FOLDERS {
        return Err(format!("At most {} external folders can be approved.", MAX_CONTROLLED_FOLDERS));
    }
    let mut safe = Vec::new();
    for raw in folders {
        let path = std::path::PathBuf::from(raw);
        if !path.is_absolute() || !is_safe_controlled_folder(&path) {
            return Err("Each folder must be an existing, non-system absolute directory.".into());
        }
        let canonical = path
            .canonicalize()
            .map_err(|_| "Unable to resolve an approved folder.")?;
        let value = canonical.to_string_lossy().to_string();
        if !safe.contains(&value) {
            safe.push(value);
        }
    }
    let path = controlled_folders_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|_| "Unable to create the local settings directory.")?;
    }
    let content = serde_json::to_string_pretty(&serde_json::json!({
        "version": 1,
        "folders": safe,
    }))
    .map_err(|_| "Unable to encode the folder settings.")?;
    std::fs::write(path, content + "\n")
        .map_err(|_| "Unable to save the approved folders.")?;
    Ok(read_controlled_folders())
}

/// Parse config text. A missing or invalid `kind` (or a missing file) falls
/// back to the serde default (Cloud); an explicit Ollama/Custom selection
/// round-trips so the local backend can boot without cloud configuration.
fn parse_inference_config(text: &str) -> InferenceConfig {
    // Parse the on-disk config verbatim. A missing or invalid `kind` field
    // falls back to the serde default (Cloud) only because there is no other
    // viable default for an absent value; a user's explicit Ollama/Custom
    // selection must round-trip unchanged so the local backend can boot.
    serde_json::from_str::<InferenceConfig>(text).unwrap_or_default()
}

/// Read the on-disk inference config, or the cloud default if absent.
fn read_inference_config() -> InferenceConfig {
    match std::fs::read_to_string(inference_config_path()) {
        Ok(text) => parse_inference_config(&text),
        Err(_) => InferenceConfig::default(),
    }
}

/// Write the inference config to disk (pretty JSON).
fn write_inference_config(cfg: &InferenceConfig) -> Result<(), String> {
    let path = inference_config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json + "\n").map_err(|e| format!("Failed to save inference config: {}", e))
}

/// Upsert `[engine.<engine>] host = "<host>"` into an existing config.toml
/// string, preserving all other content/formatting. Pure: string in, string out.
fn upsert_engine_host(existing: &str, engine: &str, host: &str) -> Result<String, String> {
    let mut doc = existing
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("Invalid config.toml: {}", e))?;
    doc["engine"][engine]["host"] = toml_edit::value(host);
    Ok(doc.to_string())
}

/// Persist the cloud privacy boundary before starting the local backend.
fn set_cloud_privacy_config(provider: &str) -> Result<(), String> {
    validate_cloud_provider(provider)?;
    let path = std::path::PathBuf::from(home_dir())
        .join(".openjarvis")
        .join("config.toml");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create configuration directory: {}", e))?;
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc = existing
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("Invalid config.toml: {}", e))?;
    doc["privacy"]["mode"] = toml_edit::value("explicit_external");
    doc["privacy"]["approved_external_providers"] = toml_edit::value(provider);
    doc["privacy"]["require_tls"] = toml_edit::value(true);
    doc["analytics"]["enabled"] = toml_edit::value(false);
    std::fs::write(&path, doc.to_string())
        .map_err(|e| format!("Failed to write cloud privacy configuration: {}", e))
}

/// Write the custom-endpoint host into ~/.openjarvis/config.toml so
/// `jarvis serve` (which reads that file via load_config) points at it.
/// The `<ENGINE>_HOST` env var is unreliable — it is shadowed by the engine's
/// non-empty default host in the Python layer — so config.toml is the override.
fn set_engine_host_in_config(engine: &str, host: &str) -> Result<(), String> {
    let path = std::path::PathBuf::from(home_dir())
        .join(".openjarvis")
        .join("config.toml");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = upsert_engine_host(&existing, engine, host)?;
    std::fs::write(&path, updated).map_err(|e| format!("Failed to write config.toml: {}", e))
}

/// Check speech backend health.
#[tauri::command]
async fn speech_health(api_url: String) -> Result<serde_json::Value, String> {
    let url = format!("{}/v1/speech/health", api_url);
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))?;
    Ok(body)
}

// ---------------------------------------------------------------------------
// Native macOS overlay — NSPanel + WKWebView, entirely bypassing Tauri's
// window management so we get proper always-on-top, transparency, non-
// activating panel behaviour and cross-Space support.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod native_overlay {
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel, BOOL, NO, YES};
    use objc::{class, msg_send, sel, sel_impl};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Raw pointer to the NSPanel, stored as usize for atomicity.
    static PANEL_PTR: AtomicUsize = AtomicUsize::new(0);
    /// Raw pointer to the WKWebView inside the panel.
    static WEBVIEW_PTR: AtomicUsize = AtomicUsize::new(0);
    /// Raw pointer to the previously-frontmost NSRunningApplication.
    static PREV_APP: AtomicUsize = AtomicUsize::new(0);

    // CoreGraphics geometry types expected by AppKit.
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGSize {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    /// Create an autoreleased NSString from a Rust &str.
    unsafe fn nsstring(s: &str) -> *mut Object {
        let obj: *mut Object = msg_send![class!(NSString), alloc];
        msg_send![obj,
            initWithBytes: s.as_ptr()
            length: s.len()
            encoding: 4usize  // NSUTF8StringEncoding
        ]
    }

    // ------------------------------------------------------------------
    // Conversation persistence
    // ------------------------------------------------------------------

    fn conversation_path() -> std::path::PathBuf {
        std::path::PathBuf::from(super::home_dir())
            .join(".openjarvis")
            .join("overlay-conversation.json")
    }

    pub fn load_conversation() -> String {
        std::fs::read_to_string(conversation_path()).unwrap_or_else(|_| "[]".into())
    }

    /// Read cloud API keys and return a JSON array of model IDs
    /// whose provider has a key configured.
    fn cloud_models_json() -> String {
        let keys = super::read_cloud_keys();
        let mut models: Vec<&str> = Vec::new();
        for (name, value) in &keys {
            if value.is_empty() {
                continue;
            }
            match name.as_str() {
                "OPENAI_API_KEY" => models.extend(["gpt-4o", "gpt-4o-mini"]),
                "ANTHROPIC_API_KEY" => {
                    models.extend(["claude-sonnet-4-20250514", "claude-haiku-4-20250414"])
                }
                "GEMINI_API_KEY" | "GOOGLE_API_KEY" => {
                    models.extend(["gemini-2.5-flash", "gemini-2.5-pro"])
                }
                _ => {}
            }
        }
        serde_json::to_string(&models).unwrap_or_else(|_| "[]".into())
    }

    fn save_conversation(json: &str) {
        let path = conversation_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, json);
    }

    /// Apply every transparency trick to the WKWebView.
    /// Called once at creation and again after the page finishes loading.
    unsafe fn force_transparent(wv: *mut Object) {
        let clear: *mut Object = msg_send![class!(NSColor), clearColor];
        let _: () = msg_send![wv, _setDrawsBackground: NO];
        let no_num: *mut Object = msg_send![class!(NSNumber), numberWithBool: NO];
        let _: () = msg_send![wv, setValue: no_num forKey: nsstring("drawsBackground")];
        let _: () = msg_send![wv, setUnderPageBackgroundColor: clear];
        // Also inject CSS to nuke any remaining background
        let js = nsstring(
            "document.documentElement.style.background='transparent';\
             document.body.style.background='transparent';"
        );
        let nil: *mut Object = std::ptr::null_mut();
        let _: () = msg_send![wv, evaluateJavaScript: js completionHandler: nil];
    }

    // ------------------------------------------------------------------
    // Public API (must be called on the main thread)
    // ------------------------------------------------------------------

    /// Build the native overlay panel.  Call once during app setup.
    pub unsafe fn create(html: &str, api_port: u16) {
        // --- Custom NSPanel subclass that accepts keyboard input ------
        if Class::get("JarvisOverlayPanel").is_none() {
            let sup = Class::get("NSPanel").unwrap();
            let mut decl = ClassDecl::new("JarvisOverlayPanel", sup).unwrap();
            extern "C" fn yes(_: &Object, _: Sel) -> BOOL {
                YES
            }
            decl.add_method(
                sel!(canBecomeKeyWindow),
                yes as extern "C" fn(&Object, Sel) -> BOOL,
            );
            decl.register();
        }

        // --- WKNavigationDelegate — re-apply transparency after load --
        if Class::get("JarvisOverlayNavDelegate").is_none() {
            let sup = Class::get("NSObject").unwrap();
            let mut decl = ClassDecl::new("JarvisOverlayNavDelegate", sup).unwrap();
            extern "C" fn did_finish(_: &Object, _: Sel, wv: *mut Object, _nav: *mut Object) {
                unsafe { force_transparent(wv); }
            }
            decl.add_method(
                sel!(webView:didFinishNavigation:),
                did_finish as extern "C" fn(&Object, Sel, *mut Object, *mut Object),
            );
            decl.register();
        }

        // --- WKScriptMessageHandler so JS can call hide() ------------
        if Class::get("JarvisOverlayMsgHandler").is_none() {
            let sup = Class::get("NSObject").unwrap();
            let mut decl = ClassDecl::new("JarvisOverlayMsgHandler", sup).unwrap();
            extern "C" fn on_msg(_: &Object, _: Sel, _ctrl: *mut Object, msg: *mut Object) {
                unsafe {
                    let body: *mut Object = msg_send![msg, body];
                    if body.is_null() {
                        return;
                    }
                    let c: *const std::os::raw::c_char = msg_send![body, UTF8String];
                    if c.is_null() {
                        return;
                    }
                    if let Ok(s) = std::ffi::CStr::from_ptr(c).to_str() {
                        if s == "hide" {
                            hide();
                        } else if let Some(json) = s.strip_prefix("save:") {
                            save_conversation(json);
                        } else if let Some(coords) = s.strip_prefix("drag:") {
                            drag(coords);
                        }
                    }
                }
            }
            decl.add_method(
                sel!(userContentController:didReceiveScriptMessage:),
                on_msg as extern "C" fn(&Object, Sel, *mut Object, *mut Object),
            );
            decl.register();
        }

        // --- Create the NSPanel --------------------------------------
        let frame = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: 560.0,
                height: 400.0,
            },
        };
        // NSWindowStyleMaskNonactivatingPanel = 1 << 7
        let style: u64 = 1 << 7;

        let cls = Class::get("JarvisOverlayPanel").unwrap();
        let panel: *mut Object = msg_send![cls, alloc];
        let panel: *mut Object = msg_send![panel,
            initWithContentRect: frame
            styleMask: style
            backing: 2u64       // NSBackingStoreBuffered
            defer: NO
        ];

        // Window level — NSFloatingWindowLevel (3).
        let _: () = msg_send![panel, setLevel: 3_i64];
        // canJoinAllSpaces (1) | fullScreenAuxiliary (1<<8)
        let _: () = msg_send![panel, setCollectionBehavior: 257_u64];
        let _: () = msg_send![panel, setHidesOnDeactivate: NO];
        let _: () = msg_send![panel, setOpaque: NO];
        let _: () = msg_send![panel, setHasShadow: NO];
        let _: () = msg_send![panel, setMovableByWindowBackground: YES];

        let clear: *mut Object = msg_send![class!(NSColor), clearColor];
        let _: () = msg_send![panel, setBackgroundColor: clear];
        let _: () = msg_send![panel, center];

        // --- WKWebView -----------------------------------------------
        let cfg: *mut Object = msg_send![class!(WKWebViewConfiguration), alloc];
        let cfg: *mut Object = msg_send![cfg, init];

        // Attach message handler ("overlay" channel)
        let hcls = Class::get("JarvisOverlayMsgHandler").unwrap();
        let handler: *mut Object = msg_send![hcls, alloc];
        let handler: *mut Object = msg_send![handler, init];
        let uc: *mut Object = msg_send![cfg, userContentController];
        let _: () = msg_send![uc,
            addScriptMessageHandler: handler
            name: nsstring("overlay")
        ];

        let wv: *mut Object = msg_send![class!(WKWebView), alloc];
        let wv: *mut Object = msg_send![wv,
            initWithFrame: frame
            configuration: cfg
        ];

        // ---- Make the webview fully transparent ----
        force_transparent(wv);

        // Set navigation delegate so we re-apply after page loads
        let nav_cls = Class::get("JarvisOverlayNavDelegate").unwrap();
        let nav_del: *mut Object = msg_send![nav_cls, alloc];
        let nav_del: *mut Object = msg_send![nav_del, init];
        let _: () = msg_send![wv, setNavigationDelegate: nav_del];

        let _: () = msg_send![panel, setContentView: wv];
        WEBVIEW_PTR.store(wv as usize, Ordering::SeqCst);

        // Inject saved conversation into the HTML template, then load it.
        // Use the API server as the base URL so fetch() is same-origin.
        // Escape "</" so the JSON can't prematurely close the <script> tag.
        // ("\/" is valid JSON — resolves back to "/" when parsed.)
        let saved = load_conversation().replace("</", "<\\/");
        let cloud = cloud_models_json();
        let filled = html
            .replace("__SAVED_MESSAGES__", &saved)
            .replace("__CLOUD_MODELS__", &cloud);
        let base_str = nsstring(&format!("http://127.0.0.1:{}", api_port));
        let base_url: *mut Object = msg_send![class!(NSURL), URLWithString: base_str];
        let _: () = msg_send![wv,
            loadHTMLString: nsstring(&filled)
            baseURL: base_url
        ];

        PANEL_PTR.store(panel as usize, Ordering::SeqCst);
    }

    pub unsafe fn toggle() {
        let ptr = PANEL_PTR.load(Ordering::SeqCst);
        if ptr == 0 {
            return;
        }
        let panel = ptr as *mut Object;
        let vis: BOOL = msg_send![panel, isVisible];
        if vis != NO {
            hide();
        } else {
            show();
        }
    }

    pub unsafe fn show() {
        let ptr = PANEL_PTR.load(Ordering::SeqCst);
        if ptr == 0 {
            return;
        }
        let panel = ptr as *mut Object;

        // Re-apply transparency every time (the webview can reset it)
        let wv_ptr = WEBVIEW_PTR.load(Ordering::SeqCst);
        if wv_ptr != 0 {
            force_transparent(wv_ptr as *mut Object);
        }

        // Remember the currently-frontmost app so we can restore it.
        let ws: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let front: *mut Object = msg_send![ws, frontmostApplication];
        if !front.is_null() {
            let _: () = msg_send![front, retain];
            let old = PREV_APP.swap(front as usize, Ordering::SeqCst);
            if old != 0 {
                let _: () = msg_send![(old as *mut Object), release];
            }
        }

        // Activate our process so the panel receives keyboard input.
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, activateIgnoringOtherApps: YES];
        let nil: *mut Object = std::ptr::null_mut();
        let _: () = msg_send![panel, makeKeyAndOrderFront: nil];

        // Focus the text field inside the webview.
        let wv: *mut Object = msg_send![panel, contentView];
        let js = nsstring("document.getElementById('input').focus()");
        let _: () = msg_send![wv, evaluateJavaScript: js completionHandler: nil];
    }

    /// Move the panel by a screen-space delta (called from JS drag handler).
    unsafe fn drag(coords: &str) {
        let ptr = PANEL_PTR.load(Ordering::SeqCst);
        if ptr == 0 {
            return;
        }
        let panel = ptr as *mut Object;
        let Some((dxs, dys)) = coords.split_once(',') else {
            return;
        };
        let Ok(dx) = dxs.parse::<f64>() else { return };
        let Ok(dy) = dys.parse::<f64>() else { return };
        // NSWindow frame origin is bottom-left; screen Y increases upward,
        // but mouse screenY increases downward, so invert dy.
        let frame: CGRect = msg_send![panel, frame];
        let origin = CGPoint {
            x: frame.origin.x + dx,
            y: frame.origin.y - dy,
        };
        let _: () = msg_send![panel, setFrameOrigin: origin];
    }

    pub unsafe fn hide() {
        let ptr = PANEL_PTR.load(Ordering::SeqCst);
        if ptr == 0 {
            return;
        }
        let panel = ptr as *mut Object;
        let nil: *mut Object = std::ptr::null_mut();
        let _: () = msg_send![panel, orderOut: nil];

        // Give focus back to whatever app was frontmost before.
        let prev = PREV_APP.swap(0, Ordering::SeqCst);
        if prev != 0 {
            let prev_app = prev as *mut Object;
            let _: BOOL = msg_send![prev_app, activateWithOptions: 2_u64];
            let _: () = msg_send![prev_app, release];
        }
    }
}

/// Dispatch a closure onto the main thread via GCD.
#[cfg(target_os = "macos")]
fn on_main_thread(f: impl FnOnce() + Send + 'static) {
    dispatch::Queue::main().exec_async(f);
}

// ---------------------------------------------------------------------------
// Overlay Tauri commands (thin wrappers that dispatch to the main thread)
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_overlay_conversation() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        return Ok(native_overlay::load_conversation());
    }
    #[cfg(not(target_os = "macos"))]
    Ok("[]".into())
}

#[tauri::command]
async fn toggle_overlay() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    on_main_thread(|| unsafe { native_overlay::toggle() });
    Ok(())
}

#[tauri::command]
async fn hide_overlay() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    on_main_thread(|| unsafe { native_overlay::hide() });
    Ok(())
}

// ---------------------------------------------------------------------------
// App entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let backend: SharedBackend = Arc::new(Mutex::new(BackendManager::default()));
    let status: SharedStatus = Arc::new(Mutex::new(SetupStatus::default()));

    let boot_backend_ref = backend.clone();
    let boot_status_ref = status.clone();

    tauri::Builder::default()
        .manage(backend.clone())
        .manage(status.clone())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .setup(move |app| {
            // System tray
            let show = MenuItemBuilder::with_id("show", "Show / Hide").build(app)?;
            let health = MenuItemBuilder::with_id("health", "Health: starting...")
                .enabled(false)
                .build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit OpenJarvis").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show)
                .separator()
                .item(&health)
                .separator()
                .item(&quit)
                .build()?;

            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("OpenJarvis")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // Create native macOS overlay panel
            #[cfg(target_os = "macos")]
            unsafe {
                native_overlay::create(include_str!("overlay.html"), JARVIS_PORT);
            }

            // Register Cmd+Shift+Space to toggle the overlay
            {
                use tauri_plugin_global_shortcut::{
                    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
                };
                let sc = Shortcut::new(Some(Modifiers::META | Modifiers::SHIFT), Code::Space);
                if let Err(e) = app.global_shortcut().on_shortcut(sc, |_app, _sc, ev| {
                    if ev.state == ShortcutState::Pressed {
                        #[cfg(target_os = "macos")]
                        unsafe {
                            native_overlay::toggle();
                        }
                    }
                }) {
                    eprintln!("Warning: could not register Cmd+Shift+Space: {e}");
                }
            }

            // Auto-start backend services on launch
            tauri::async_runtime::spawn(boot_backend(boot_backend_ref, boot_status_ref));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_setup_status,
            get_api_base,
            start_backend,
            stop_backend,
            check_health,
            run_secure_self_test,
            fetch_energy,
            fetch_telemetry,
            fetch_traces,
            fetch_trace,
            fetch_learning_stats,
            fetch_learning_policy,
            fetch_memory_stats,
            search_memory,
            fetch_agents,
            fetch_models,
            fetch_savings,
            submit_savings,
            transcribe_audio,
            speech_health,
            save_cloud_key,
            get_cloud_key_status,
            get_inference_source,
            set_inference_source,
            get_transcription_source,
            set_transcription_source,
            get_transcription_status,
            get_gemini_live_config,
            set_gemini_live_config,
            mint_gemini_live_session_token,
            get_android_adb_config,
            set_android_adb_config,
            discover_android_adb_devices,
            get_controlled_folders,
            set_controlled_folders,
            toggle_overlay,
            hide_overlay,
            get_overlay_conversation,
        ])
        .build(tauri::generate_context!())
        .expect("error while building OpenJarvis Desktop")
        .run(move |_app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let b = backend.clone();
                tauri::async_runtime::spawn(async move {
                    b.lock().await.stop_all().await;
                });
            }
        });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{
        boot_plan, default_local_model, format_extension_import_failure,
        format_missing_rust_toolchain, format_port_unavailable, format_uv_sync_failure,
        format_uv_sync_spawn_error, matching_installed_model, model_names_match,
        gemini_live_token_payload, parse_inference_config, parse_ollama_model_names,
        preferred_installed_model, secure_self_test_check, secure_self_test_report,
        should_persist_resolved_model,
        startup_installed_model, upsert_engine_host, uv_sync_stderr_tail, InferenceConfig,
        SourceKind, DESKTOP_UV_SYNC_COMMAND,
    };
    use std::path::Path;

    #[test]
    fn gemini_live_token_payload_is_one_use_audio_only_and_model_constrained() {
        let payload = gemini_live_token_payload();

        assert_eq!(payload["uses"], 1);
        assert_eq!(
            payload["liveConnectConstraints"]["model"],
            "models/gemini-3.1-flash-live-preview"
        );
        assert_eq!(
            payload["liveConnectConstraints"]["config"]["responseModalities"],
            serde_json::json!(["AUDIO"])
        );
        assert!(payload["liveConnectConstraints"]["config"]
            .get("sessionResumption")
            .is_some());
    }

    #[test]
    fn secure_self_test_counts_only_declared_statuses() {
        let report = secure_self_test_report(vec![
            secure_self_test_check("cloud", "pass", "Cloud", "Ready"),
            secure_self_test_check("voice", "not_configured", "Voice", "Missing"),
            secure_self_test_check(
                "desktop",
                "live_check_required",
                "Desktop",
                "Manual verification required",
            ),
        ]);

        assert_eq!(report.passed, 1);
        assert_eq!(report.warnings, 1);
        assert_eq!(report.live_checks_required, 1);
        assert_eq!(report.checks.len(), 3);
    }

    #[test]
    fn tail_returns_whole_string_when_shorter_than_limit() {
        assert_eq!(uv_sync_stderr_tail("short error", 800), "short error");
    }

    #[test]
    fn tail_keeps_the_end_not_the_beginning() {
        // uv's actionable line is at the end; the spinner noise is at the start.
        let s = format!("{}ACTUAL ERROR HERE", "spinner-noise ".repeat(200));
        let tail = uv_sync_stderr_tail(&s, 40);
        assert!(tail.ends_with("ACTUAL ERROR HERE"), "tail was: {tail:?}");
        assert!(!tail.contains("spinner-noise spinner-noise spinner-noise"));
        assert!(tail.chars().count() <= 40);
    }

    #[test]
    fn tail_trims_surrounding_whitespace() {
        assert_eq!(uv_sync_stderr_tail("  \n padded \n  ", 800), "padded");
    }

    #[test]
    fn tail_never_splits_a_multibyte_codepoint() {
        // Each "é" is 2 bytes / 1 char. A byte-based slice could panic or
        // produce invalid UTF-8; the char-based tail must not.
        let s = "é".repeat(500);
        let tail = uv_sync_stderr_tail(&s, 100);
        assert_eq!(tail.chars().count(), 100);
        assert!(tail.chars().all(|c| c == 'é'));
    }

    #[test]
    fn failure_message_includes_exit_code_and_tail_and_hint() {
        let msg = format_uv_sync_failure(
            Path::new("/home/u/.openjarvis/src"),
            Some(2),
            "error: failed to resolve numpy==2.1.3",
        );
        assert!(msg.contains("exit 2"));
        assert!(msg.contains("/home/u/.openjarvis/src"));
        assert!(msg.contains("failed to resolve numpy==2.1.3"));
        assert!(msg.contains(DESKTOP_UV_SYNC_COMMAND)); // actionable next step
    }

    #[test]
    fn failure_message_renders_missing_exit_code_as_unknown() {
        // Process killed by signal → no exit code. Must not show a misleading -1.
        let msg = format_uv_sync_failure(Path::new("/x"), None, "boom");
        assert!(msg.contains("exit unknown"));
        assert!(!msg.contains("exit -1"));
    }

    #[test]
    fn spawn_error_names_the_binary_and_root() {
        let msg = format_uv_sync_spawn_error(
            Path::new("/repo"),
            "C:\\Users\\me\\.local\\bin\\uv.exe",
            "No such file or directory (os error 2)",
        );
        assert!(msg.contains("C:\\Users\\me\\.local\\bin\\uv.exe"));
        assert!(msg.contains("/repo"));
        assert!(msg.contains("No such file or directory"));
    }

    #[test]
    fn missing_rust_toolchain_message_names_cargo_and_installer() {
        let msg = format_missing_rust_toolchain();
        assert!(msg.contains("cargo"));
        assert!(msg.contains("https://rustup.rs"));
        assert!(msg.contains("openjarvis_rust"));
        assert!(msg.contains("Visual Studio Build Tools"));
    }

    #[test]
    fn uv_sync_rust_failure_mentions_toolchain() {
        let msg = format_uv_sync_failure(
            Path::new("C:\\Users\\me\\OpenJarvis"),
            Some(1),
            "maturin failed: linker `link.exe` not found while building openjarvis-rust",
        );
        assert!(msg.contains("exit 1"));
        assert!(msg.contains("link.exe"));
        assert!(msg.contains("https://rustup.rs"));
        assert!(msg.contains("Visual Studio Build Tools"));
    }

    #[test]
    fn extension_import_failure_names_verification_command() {
        let msg = format_extension_import_failure(
            Path::new("C:\\Users\\me\\OpenJarvis"),
            "ModuleNotFoundError: No module named 'openjarvis_rust'",
        );
        assert!(msg.contains("openjarvis_rust"));
        assert!(msg.contains(DESKTOP_UV_SYNC_COMMAND));
        assert!(msg.contains("uv run python -c \"import openjarvis_rust\""));
        assert!(msg.contains("ModuleNotFoundError"));
    }

    #[test]
    fn port_unavailable_message_names_port_and_owner_hint() {
        let msg = format_port_unavailable(8000, "address already in use");
        assert!(msg.contains("Port 8000 is not available"));
        assert!(msg.contains("address already in use"));
        assert!(msg.contains("To identify it"));
        assert!(msg.contains("8000"));
    }

    #[test]
    fn default_local_model_picks_second_largest_that_fits() {
        // QWEN35_MODELS min_ram ladder: 4,6,8,12,24,32,96 GB
        assert_eq!(default_local_model(4.0), "qwen3.5:0.8b");  // only one fits
        assert_eq!(default_local_model(8.0), "qwen3.5:2b");    // fits 0.8/2/4 → 2nd-largest
        assert_eq!(default_local_model(16.0), "qwen3.5:4b");   // fits ..9b → 2nd-largest
        assert_eq!(default_local_model(32.0), "qwen3.5:27b");  // fits 0.8/2/4/9/27/35b → 2nd-largest is 27b
        assert_eq!(default_local_model(128.0), "qwen3.5:35b"); // fits all → 2nd-largest
    }

    #[test]
    fn default_local_model_falls_back_when_nothing_fits() {
        assert_eq!(default_local_model(1.0), super::FALLBACK_MODEL);
    }

    #[test]
    fn parse_ollama_model_names_reads_nonempty_names() {
        let body = serde_json::json!({
            "models": [
                {"name": "llama3.2:latest"},
                {"name": ""},
                {"name": "qwen3.5:4b"},
                {"model": "mistral:latest"}
            ]
        });
        assert_eq!(
            parse_ollama_model_names(&body),
            vec![
                "llama3.2:latest".to_string(),
                "qwen3.5:4b".to_string(),
                "mistral:latest".to_string()
            ]
        );
    }

    #[test]
    fn model_names_match_treats_latest_as_optional() {
        assert!(model_names_match("llama3.2:latest", "llama3.2"));
        assert!(model_names_match("llama3.2", "llama3.2:latest"));
        assert!(model_names_match("qwen3.5:4b", "qwen3.5:4b"));
        assert!(!model_names_match("llama3.2:latest", "qwen3.5:4b"));
    }

    #[test]
    fn installed_model_helpers_pick_matching_or_first_model() {
        let models = vec!["llama3.2:latest".to_string(), "qwen3.5:4b".to_string()];
        assert_eq!(
            matching_installed_model(&models, "llama3.2"),
            Some("llama3.2:latest".to_string())
        );
        assert_eq!(
            preferred_installed_model(&models),
            Some("llama3.2:latest".to_string())
        );
    }

    #[test]
    fn preferred_installed_model_skips_embedding_names_when_chat_model_exists() {
        let models = vec![
            "nomic-embed-text:latest".to_string(),
            "llama3.2:latest".to_string(),
        ];
        assert_eq!(
            preferred_installed_model(&models),
            Some("llama3.2:latest".to_string())
        );
    }

    #[test]
    fn startup_installed_model_uses_existing_model_for_defaults() {
        let models = vec!["llama3.2:latest".to_string()];
        assert_eq!(
            startup_installed_model("qwen3.5:4b", &models),
            Some("llama3.2:latest".to_string())
        );
    }

    #[test]
    fn startup_installed_model_uses_existing_model_when_configured_model_missing() {
        let models = vec!["llama3.2:latest".to_string()];
        assert_eq!(
            startup_installed_model("qwen3.5:4b", &models),
            Some("llama3.2:latest".to_string())
        );
    }

    #[test]
    fn resolved_model_is_only_persisted_when_no_model_was_configured() {
        let default_cfg = InferenceConfig { kind: SourceKind::Ollama, ..Default::default() };
        assert!(should_persist_resolved_model(&default_cfg));

        let empty_cfg = InferenceConfig {
            kind: SourceKind::Ollama,
            model: Some(" ".into()),
            ..Default::default()
        };
        assert!(should_persist_resolved_model(&empty_cfg));

        let user_cfg = InferenceConfig {
            kind: SourceKind::Ollama,
            model: Some("qwen3.5:9b".into()),
            ..Default::default()
        };
        assert!(!should_persist_resolved_model(&user_cfg));
    }

    #[test]
    fn parse_defaults_to_cloud_when_file_missing_or_garbage() {
        assert!(matches!(parse_inference_config("").kind, SourceKind::Cloud));
        assert!(matches!(parse_inference_config("not json").kind, SourceKind::Cloud));
    }

    #[test]
    fn parse_local_config_round_trips_custom() {
        let cfg = parse_inference_config(
            r#"{"kind":"custom","model":"qwen2.5-7b","host":"http://localhost:1234","engine":"lmstudio"}"#,
        );
        assert!(matches!(cfg.kind, SourceKind::Custom));
        assert_eq!(cfg.model.as_deref(), Some("qwen2.5-7b"));
        assert_eq!(cfg.host.as_deref(), Some("http://localhost:1234"));
        assert_eq!(cfg.engine.as_deref(), Some("lmstudio"));
    }

    #[test]
    fn parse_local_config_round_trips_ollama() {
        let cfg = parse_inference_config(r#"{"kind":"ollama","model":"qwen3.5:4b"}"#);
        assert!(matches!(cfg.kind, SourceKind::Ollama));
        assert_eq!(cfg.model.as_deref(), Some("qwen3.5:4b"));
    }

    #[test]
    fn parse_cloud_profile_preserves_authorized_provider() {
        let cfg = parse_inference_config(
            r#"{"kind":"cloud","provider":"openai","model":"gpt-5-mini"}"#,
        );
        assert!(matches!(cfg.kind, SourceKind::Cloud));
        assert_eq!(cfg.provider.as_deref(), Some("openai"));
        assert_eq!(cfg.model.as_deref(), Some("gpt-5-mini"));
    }

    #[test]
    fn boot_plan_ollama_launches_and_pulls_one_model() {
        let cfg = InferenceConfig { kind: SourceKind::Ollama, ..Default::default() };
        let plan = boot_plan(&cfg, 16.0);
        assert!(plan.launch_ollama);
        assert_eq!(plan.model_to_pull.as_deref(), Some("qwen3.5:4b"));
        assert!(plan.engine_host.is_none());
        assert!(plan.serve_args.windows(2).any(|w| w == ["--engine", "ollama"]));
        assert!(plan.serve_args.windows(2).any(|w| w == ["--model", "qwen3.5:4b"]));
    }

    #[test]
    fn boot_plan_ollama_respects_pinned_model() {
        let cfg = InferenceConfig {
            kind: SourceKind::Ollama,
            model: Some("qwen3.5:9b".into()),
            ..Default::default()
        };
        let plan = boot_plan(&cfg, 16.0);
        assert_eq!(plan.model_to_pull.as_deref(), Some("qwen3.5:9b"));
    }

    #[test]
    fn boot_plan_custom_skips_ollama_and_sets_engine_host() {
        let cfg = InferenceConfig {
            kind: SourceKind::Custom,
            model: Some("qwen2.5-7b".into()),
            host: Some("http://localhost:1234".into()),
            engine: Some("lmstudio".into()),
            provider: None,
            provider_endpoint: None,
            provider_processing_acknowledged: false,
        };
        let plan = boot_plan(&cfg, 16.0);
        assert!(!plan.launch_ollama);
        assert!(plan.model_to_pull.is_none());
        assert_eq!(
            plan.engine_host,
            Some(("lmstudio".to_string(), "http://localhost:1234".to_string()))
        );
        assert!(plan.serve_args.windows(2).any(|w| w == ["--engine", "lmstudio"]));
        assert!(plan.serve_args.windows(2).any(|w| w == ["--model", "qwen2.5-7b"]));
    }

    #[test]
    fn boot_plan_custom_defaults_engine_to_lmstudio() {
        let cfg = InferenceConfig {
            kind: SourceKind::Custom,
            model: Some("m".into()),
            host: Some("http://h:1".into()),
            engine: None,
            provider: None,
            provider_endpoint: None,
            provider_processing_acknowledged: false,
        };
        let plan = boot_plan(&cfg, 16.0);
        assert_eq!(plan.engine_host.as_ref().unwrap().0, "lmstudio");
        assert!(plan.serve_args.windows(2).any(|w| w == ["--engine", "lmstudio"]));
    }

    #[test]
    fn boot_plan_custom_omits_engine_host_when_no_host() {
        // No configured host → don't set engine_host (no override to write).
        let cfg = InferenceConfig {
            kind: SourceKind::Custom,
            model: Some("m".into()),
            host: None,
            engine: Some("lmstudio".into()),
            provider: None,
            provider_endpoint: None,
            provider_processing_acknowledged: false,
        };
        let plan = boot_plan(&cfg, 16.0);
        assert!(plan.engine_host.is_none());
    }

    #[test]
    fn boot_plan_ollama_uses_fallback_model_on_low_ram() {
        // Below the smallest model's min_ram → default_local_model → FALLBACK_MODEL.
        let cfg = InferenceConfig { kind: SourceKind::Ollama, ..Default::default() };
        let plan = boot_plan(&cfg, 1.0);
        assert_eq!(plan.model_to_pull.as_deref(), Some(super::FALLBACK_MODEL));
    }

    #[test]
    fn upsert_engine_host_writes_into_empty_config() {
        let out = upsert_engine_host("", "lmstudio", "http://localhost:1234").unwrap();
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        assert_eq!(
            doc["engine"]["lmstudio"]["host"].as_str(),
            Some("http://localhost:1234")
        );
    }

    #[test]
    fn upsert_engine_host_preserves_existing_content() {
        let existing = "[intelligence]\ndefault_model = \"keep-me\"\n";
        let out = upsert_engine_host(existing, "vllm", "http://host:8000").unwrap();
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        assert_eq!(doc["intelligence"]["default_model"].as_str(), Some("keep-me"));
        assert_eq!(doc["engine"]["vllm"]["host"].as_str(), Some("http://host:8000"));
    }

    #[test]
    fn upsert_engine_host_updates_existing_host() {
        let existing = "[engine.lmstudio]\nhost = \"http://old:1\"\n";
        let out = upsert_engine_host(existing, "lmstudio", "http://new:2").unwrap();
        let doc: toml_edit::DocumentMut = out.parse().unwrap();
        assert_eq!(doc["engine"]["lmstudio"]["host"].as_str(), Some("http://new:2"));
    }

    // -----------------------------------------------------------------
    // #455 — AppImage subprocess env-strip helper
    // -----------------------------------------------------------------
    //
    // `prepare_subprocess_for_appimage` strips LD_LIBRARY_PATH (and the
    // related AppImage runtime variables) from a child `Command` ONLY
    // when the parent process is itself running inside an AppImage —
    // detected by the presence of the `APPIMAGE` env variable that the
    // AppImage runtime sets to the original .AppImage path. We can't
    // observe the env_remove calls directly through tokio's Command
    // API (it doesn't expose its env map publicly), so these tests
    // exercise the documented contract on each platform:
    //
    //   * on macOS / Windows: the function is a no-op regardless of env.
    //   * on Linux without $APPIMAGE: also a no-op.
    //   * on Linux with $APPIMAGE: it doesn't panic, doesn't return an
    //     error, and the calling code that follows succeeds. The
    //     observable behaviour test is the integration repro on a real
    //     AppImage build (covered in PR test plan).
    //
    // The Mutex serialises any test that touches the process-wide
    // `APPIMAGE` env var so cargo test's parallel runner can't race two
    // tests setting and unsetting it concurrently. `static Mutex` works
    // on a const path since Rust 1.63 (and Tauri's MSRV is well above).

    static APPIMAGE_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    // Pick a binary that exists on every test target so the test body is
    // doing something other than constructing an obviously-broken command
    // path on Windows.
    #[cfg(target_os = "windows")]
    const HARMLESS_BIN: &str = "cmd";
    #[cfg(not(target_os = "windows"))]
    const HARMLESS_BIN: &str = "/bin/true";

    #[test]
    fn prepare_subprocess_for_appimage_no_appimage_is_safe() {
        let _guard = APPIMAGE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("APPIMAGE");
        // SAFETY: APPIMAGE_ENV_LOCK serialises every test that touches
        // this env var, so the mutation is single-threaded for the
        // duration of the lock. The 2024-edition env mutation rules
        // require the `unsafe` block but the guard makes it sound.
        unsafe {
            std::env::remove_var("APPIMAGE");
        }
        let mut cmd = tokio::process::Command::new(HARMLESS_BIN);
        super::prepare_subprocess_for_appimage(&mut cmd);
        if let Some(v) = prev {
            unsafe {
                std::env::set_var("APPIMAGE", v);
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prepare_subprocess_for_appimage_with_appimage_set_is_safe() {
        let _guard = APPIMAGE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("APPIMAGE");
        unsafe {
            std::env::set_var("APPIMAGE", "/tmp/test.AppImage");
        }
        let mut cmd = tokio::process::Command::new(HARMLESS_BIN);
        super::prepare_subprocess_for_appimage(&mut cmd);
        unsafe {
            if let Some(v) = prev {
                std::env::set_var("APPIMAGE", v);
            } else {
                std::env::remove_var("APPIMAGE");
            }
        }
    }
}
