//! Quiet, privacy-preserving diagnostics for the optional YouTube Music runtime.
//!
//! This module intentionally has no UI. It keeps a small in-memory snapshot that
//! a future Settings panel can read. Routine failures are not surfaced in the
//! player and reports never include cookies, request headers or stream URLs.

use crate::youtube::YouTubeBridge;
use gtk::glib;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{OnceLock, RwLock},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const BACKGROUND_INTERVAL: Duration = Duration::from_secs(15 * 60);
const COMMAND_TIMEOUT_NOTE: &str = "version probe unavailable";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiagnosticState {
    #[default]
    Unknown,
    Ok,
    Warning,
    Error,
}

impl DiagnosticState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Ok => "ok",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiagnosticCheck {
    pub state: DiagnosticState,
    pub summary: String,
    pub detail: String,
}

impl DiagnosticCheck {
    fn ok(summary: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            state: DiagnosticState::Ok,
            summary: summary.into(),
            detail: detail.into(),
        }
    }

    fn warning(summary: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            state: DiagnosticState::Warning,
            summary: summary.into(),
            detail: detail.into(),
        }
    }

    fn error(summary: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            state: DiagnosticState::Error,
            summary: summary.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct YouTubeDiagnosticsSnapshot {
    pub checked_at_unix: u64,
    pub helper: DiagnosticCheck,
    pub python_runtime: DiagnosticCheck,
    pub ytmusicapi: DiagnosticCheck,
    pub yt_dlp: DiagnosticCheck,
    pub deno: DiagnosticCheck,
    pub account: DiagnosticCheck,
    pub cache: DiagnosticCheck,
}

impl YouTubeDiagnosticsSnapshot {
    pub fn overall_state(&self) -> DiagnosticState {
        let checks = [
            &self.helper,
            &self.python_runtime,
            &self.ytmusicapi,
            &self.yt_dlp,
            &self.deno,
            &self.account,
            &self.cache,
        ];

        if checks
            .iter()
            .any(|check| check.state == DiagnosticState::Error)
        {
            DiagnosticState::Error
        } else if checks
            .iter()
            .any(|check| check.state == DiagnosticState::Warning)
        {
            DiagnosticState::Warning
        } else if checks
            .iter()
            .all(|check| check.state == DiagnosticState::Ok)
        {
            DiagnosticState::Ok
        } else {
            DiagnosticState::Unknown
        }
    }

    pub fn sanitized_report(&self) -> String {
        let mut report = String::new();
        report.push_str("Nocky YouTube Music diagnostics\n");
        report.push_str(&format!("checked_at_unix={}\n", self.checked_at_unix));
        report.push_str(&format!("overall={}\n", self.overall_state().label()));

        for (name, check) in [
            ("helper", &self.helper),
            ("python_runtime", &self.python_runtime),
            ("ytmusicapi", &self.ytmusicapi),
            ("yt_dlp", &self.yt_dlp),
            ("deno", &self.deno),
            ("account", &self.account),
            ("cache", &self.cache),
        ] {
            report.push_str(&format!(
                "{name}={} | {} | {}\n",
                check.state.label(),
                sanitize_text(&check.summary),
                sanitize_text(&check.detail)
            ));
        }

        report
    }
}

static SNAPSHOT: OnceLock<RwLock<YouTubeDiagnosticsSnapshot>> = OnceLock::new();
static STARTED: OnceLock<()> = OnceLock::new();

fn snapshot_lock() -> &'static RwLock<YouTubeDiagnosticsSnapshot> {
    SNAPSHOT.get_or_init(|| RwLock::new(YouTubeDiagnosticsSnapshot::default()))
}

pub fn snapshot() -> YouTubeDiagnosticsSnapshot {
    snapshot_lock()
        .read()
        .map(|snapshot| snapshot.clone())
        .unwrap_or_default()
}

pub fn sanitized_report() -> String {
    snapshot().sanitized_report()
}

/// Runs a fresh diagnostics pass without blocking GTK.
pub fn refresh_now() {
    thread::Builder::new()
        .name("nocky-youtube-diagnostics-refresh".to_string())
        .spawn(|| {
            let result = run_checks();
            if let Ok(mut snapshot) = snapshot_lock().write() {
                *snapshot = result;
            }
        })
        .unwrap_or_else(|error| {
            eprintln!("Could not refresh YouTube diagnostics: {error}");
            thread::spawn(|| {})
        });
}

/// Starts one detached diagnostics worker for the process.
///
/// The first pass runs after a short startup delay to avoid competing with the
/// initial GTK paint and library restoration. Further passes are intentionally
/// infrequent and remain silent.
pub fn start_background_checks() {
    if STARTED.set(()).is_err() {
        return;
    }

    thread::Builder::new()
        .name("nocky-youtube-diagnostics".to_string())
        .spawn(|| {
            thread::sleep(Duration::from_secs(3));
            loop {
                let result = run_checks();
                if let Ok(mut snapshot) = snapshot_lock().write() {
                    *snapshot = result;
                }
                thread::sleep(BACKGROUND_INTERVAL);
            }
        })
        .unwrap_or_else(|error| {
            eprintln!("Could not start YouTube diagnostics worker: {error}");
            thread::spawn(|| {})
        });
}

fn run_checks() -> YouTubeDiagnosticsSnapshot {
    let checked_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();

    let helper_path = discover_helper();
    let python_path = discover_python();
    let deno_path = discover_deno();

    let helper = match helper_path.as_ref() {
        Some(path) => DiagnosticCheck::ok("YouTube helper found", display_path(path)),
        None => DiagnosticCheck::error(
            "YouTube helper missing",
            "Reinstall Nocky or restore helpers/nocky_youtube.py",
        ),
    };

    let python_runtime = match python_path.as_ref() {
        Some(path) => {
            let version = command_output(path, &["--version"])
                .unwrap_or_else(|| COMMAND_TIMEOUT_NOTE.to_string());
            DiagnosticCheck::ok("Python runtime available", version)
        }
        None => DiagnosticCheck::error(
            "Python runtime missing",
            "Run scripts/setup-youtube-runtime.sh or reinstall YouTube support",
        ),
    };

    let ytmusicapi = python_module_check(python_path.as_deref(), "ytmusicapi");
    let yt_dlp = python_module_check(python_path.as_deref(), "yt_dlp");

    let deno = match deno_path.as_ref() {
        Some(path) => {
            let version = command_output(path, &["--version"])
                .and_then(|output| output.lines().next().map(str::to_owned))
                .unwrap_or_else(|| COMMAND_TIMEOUT_NOTE.to_string());
            DiagnosticCheck::ok("Deno available", version)
        }
        None => DiagnosticCheck::warning(
            "Deno not found",
            "Stream extraction may fail with current YouTube responses",
        ),
    };

    let account = match YouTubeBridge::discover() {
        Ok(bridge) => match bridge.status() {
            Ok(status) if status.connected => DiagnosticCheck::ok(
                "YouTube Music account connected",
                if status.storage.trim().is_empty() {
                    "Session storage available".to_string()
                } else {
                    sanitize_text(&status.storage)
                },
            ),
            Ok(_) => DiagnosticCheck::warning(
                "YouTube Music account not connected",
                "Public search remains available",
            ),
            Err(error) => {
                DiagnosticCheck::warning("Could not verify account state", classify_error(&error))
            }
        },
        Err(error) => DiagnosticCheck::error("YouTube bridge unavailable", classify_error(&error)),
    };

    let cache = inspect_cache();

    YouTubeDiagnosticsSnapshot {
        checked_at_unix,
        helper,
        python_runtime,
        ytmusicapi,
        yt_dlp,
        deno,
        account,
        cache,
    }
}

fn python_module_check(python: Option<&Path>, module: &str) -> DiagnosticCheck {
    let Some(python) = python else {
        return DiagnosticCheck::error(
            format!("{module} unavailable"),
            "Python runtime was not found",
        );
    };

    let script = format!(
        "import importlib.metadata as m; print(m.version('{}'))",
        if module == "yt_dlp" { "yt-dlp" } else { module }
    );
    match command_output(python, &["-c", &script]) {
        Some(version) if !version.trim().is_empty() => {
            DiagnosticCheck::ok(format!("{module} available"), version.trim())
        }
        _ => DiagnosticCheck::error(
            format!("{module} missing"),
            "Recreate the Nocky YouTube runtime",
        ),
    }
}

fn discover_helper() -> Option<PathBuf> {
    candidate_paths([
        env::var_os("NOCKY_YOUTUBE_HELPER").map(PathBuf::from),
        Some(PathBuf::from("helpers/nocky_youtube.py")),
        env::current_exe().ok().and_then(|path| {
            path.parent()
                .map(|parent| parent.join("../share/nocky/helpers/nocky_youtube.py"))
        }),
        env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".local/share/nocky/helpers/nocky_youtube.py")),
        Some(PathBuf::from(
            "/usr/local/share/nocky/helpers/nocky_youtube.py",
        )),
        Some(PathBuf::from("/usr/share/nocky/helpers/nocky_youtube.py")),
    ])
}

fn discover_python() -> Option<PathBuf> {
    let runtime_override = env::var_os("NOCKY_RUNTIME_DIR").map(PathBuf::from);
    candidate_paths([
        env::var_os("NOCKY_YOUTUBE_PYTHON").map(PathBuf::from),
        runtime_override
            .as_ref()
            .map(|path| path.join("bin/python3")),
        Some(PathBuf::from(".nocky-runtime/bin/python3")),
        env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".local/share/nocky/runtime/bin/python3")),
        Some(PathBuf::from("/usr/local/share/nocky/runtime/bin/python3")),
        Some(PathBuf::from("/usr/share/nocky/runtime/bin/python3")),
    ])
    .or_else(|| command_in_path("python3"))
}

fn discover_deno() -> Option<PathBuf> {
    let runtime_override = env::var_os("NOCKY_RUNTIME_DIR").map(PathBuf::from);
    candidate_paths([
        env::var_os("NOCKY_DENO").map(PathBuf::from),
        runtime_override.as_ref().map(|path| path.join("bin/deno")),
        Some(PathBuf::from(".nocky-runtime/bin/deno")),
        env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".local/share/nocky/runtime/bin/deno")),
        Some(PathBuf::from("/usr/local/share/nocky/runtime/bin/deno")),
        Some(PathBuf::from("/usr/share/nocky/runtime/bin/deno")),
    ])
    .or_else(|| command_in_path("deno"))
}

fn candidate_paths<const N: usize>(paths: [Option<PathBuf>; N]) -> Option<PathBuf> {
    paths.into_iter().flatten().find(|path| path.is_file())
}

fn command_in_path(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join(command))
        .find(|candidate| candidate.is_file())
}

fn command_output(program: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let value = if stdout.is_empty() { stderr } else { stdout };
    (!value.is_empty()).then_some(value)
}

fn inspect_cache() -> DiagnosticCheck {
    let root = glib::user_cache_dir().join("nocky").join("youtube");
    if !root.is_dir() {
        return DiagnosticCheck::warning(
            "YouTube cache is empty",
            "Content will be cached after YouTube Music is used",
        );
    }

    let mut files = 0_u64;
    let mut bytes = 0_u64;
    let mut newest = 0_u64;
    inspect_directory(&root, &mut files, &mut bytes, &mut newest, 0);

    if files == 0 {
        DiagnosticCheck::warning("YouTube cache is empty", display_path(&root))
    } else {
        DiagnosticCheck::ok(
            "YouTube cache available",
            format!(
                "{files} files · {} MiB · newest_mtime={newest}",
                bytes / (1024 * 1024)
            ),
        )
    }
}

fn inspect_directory(
    path: &Path,
    files: &mut u64,
    bytes: &mut u64,
    newest: &mut u64,
    depth: usize,
) {
    if depth > 6 {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        if metadata.is_dir() {
            inspect_directory(&path, files, bytes, newest, depth + 1);
            continue;
        }

        if metadata.is_file() {
            *files = files.saturating_add(1);
            *bytes = bytes.saturating_add(metadata.len());
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or_default();
            *newest = (*newest).max(modified);
        }
    }
}

fn display_path(path: &Path) -> String {
    let home = env::var_os("HOME").map(PathBuf::from);
    if let Some(home) = home {
        if let Ok(relative) = path.strip_prefix(home) {
            return format!("~/{}", relative.display());
        }
    }
    path.display().to_string()
}

fn classify_error(error: &str) -> String {
    let normalized = error.to_ascii_lowercase();
    if normalized.contains("runtime") || normalized.contains("python") {
        "Python runtime problem".to_string()
    } else if normalized.contains("helper") {
        "YouTube helper problem".to_string()
    } else if normalized.contains("network")
        || normalized.contains("connect")
        || normalized.contains("timeout")
    {
        "Network check failed".to_string()
    } else if normalized.contains("session")
        || normalized.contains("auth")
        || normalized.contains("401")
        || normalized.contains("403")
    {
        "Authentication check failed".to_string()
    } else {
        "YouTube Music check failed".to_string()
    }
}

fn sanitize_text(value: &str) -> String {
    value
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.contains("cookie")
                || lower.contains("authorization")
                || lower.contains("x-goog-authuser")
                || lower.contains("stream_url")
                || lower.contains("http_headers")
                || lower.contains("https://")
                || lower.contains("http://")
            {
                "[redacted]".to_string()
            } else {
                line.trim().to_string()
            }
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overall_state_prioritizes_errors() {
        let snapshot = YouTubeDiagnosticsSnapshot {
            helper: DiagnosticCheck::ok("ok", ""),
            deno: DiagnosticCheck::warning("warning", ""),
            python_runtime: DiagnosticCheck::error("error", ""),
            ..Default::default()
        };
        assert_eq!(snapshot.overall_state(), DiagnosticState::Error);
    }

    #[test]
    fn sanitized_report_redacts_sensitive_lines_and_urls() {
        let snapshot = YouTubeDiagnosticsSnapshot {
            account: DiagnosticCheck::warning(
                "Cookie: secret",
                "https://example.test/temporary-stream",
            ),
            ..Default::default()
        };
        let report = snapshot.sanitized_report();
        assert!(!report.contains("secret"));
        assert!(!report.contains("example.test"));
        assert!(report.contains("[redacted]"));
    }

    #[test]
    fn display_path_hides_home_prefix() {
        if let Some(home) = env::var_os("HOME") {
            let path = PathBuf::from(home).join(".cache/nocky/youtube");
            assert!(display_path(&path).starts_with("~/"));
        }
    }
}
