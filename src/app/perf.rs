//! Lightweight opt-in performance tracing.
//!
//! The helpers in this module intentionally stay dependency-free and silent by
//! default. Set `NOCKY_PERF_TRACE=1` to print compact timing lines to stderr.

use std::{env, sync::OnceLock, time::Instant};

static PERF_TRACE_ENABLED: OnceLock<bool> = OnceLock::new();

pub(crate) fn enabled() -> bool {
    *PERF_TRACE_ENABLED.get_or_init(|| {
        env::var("NOCKY_PERF_TRACE")
            .map(|value| trace_flag_enabled(&value))
            .unwrap_or(false)
    })
}

pub(crate) fn log_event(event: &'static str, fields: &[(&str, String)]) {
    if enabled() {
        emit(event, None, fields);
    }
}

#[must_use]
pub(crate) struct PerfTimer {
    event: &'static str,
    start: Option<Instant>,
}

impl PerfTimer {
    pub(crate) fn start(event: &'static str) -> Self {
        Self {
            event,
            start: enabled().then(Instant::now),
        }
    }

    pub(crate) fn finish_with(mut self, fields: &[(&str, String)]) {
        if let Some(start) = self.start.take() {
            emit(self.event, Some(start.elapsed().as_millis()), fields);
        }
    }
}

impl Drop for PerfTimer {
    fn drop(&mut self) {
        if let Some(start) = self.start.take() {
            emit(self.event, Some(start.elapsed().as_millis()), &[]);
        }
    }
}

fn emit(event: &str, duration_ms: Option<u128>, fields: &[(&str, String)]) {
    let mut line = format!("[perf] event={event}");
    if let Some(duration_ms) = duration_ms {
        line.push_str(" duration_ms=");
        line.push_str(&duration_ms.to_string());
    }

    for (key, value) in fields {
        line.push(' ');
        line.push_str(key);
        line.push('=');
        append_field_value(&mut line, value);
    }

    eprintln!("{line}");
}

fn append_field_value(line: &mut String, value: &str) {
    if value.chars().any(char::is_whitespace) {
        line.push('"');
        for ch in value.chars() {
            match ch {
                '\\' => line.push_str("\\\\"),
                '"' => line.push_str("\\\""),
                _ => line.push(ch),
            }
        }
        line.push('"');
    } else {
        line.push_str(value);
    }
}

fn trace_flag_enabled(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::trace_flag_enabled;

    #[test]
    fn recognizes_truthy_trace_values() {
        for value in ["1", "true", "TRUE", "yes", "on", " on "] {
            assert!(trace_flag_enabled(value));
        }
    }

    #[test]
    fn rejects_falsey_or_empty_trace_values() {
        for value in ["", "0", "false", "no", "off", "disabled"] {
            assert!(!trace_flag_enabled(value));
        }
    }
}
