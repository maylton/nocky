use std::{fs, path::Path};

#[derive(Clone, Debug)]
pub struct LyricLine {
    pub timestamp_us: i64,
    pub text: String,
}

pub fn load_sidecar(audio_path: &Path) -> Vec<LyricLine> {
    let lrc_path = audio_path.with_extension("lrc");
    let Ok(contents) = fs::read_to_string(lrc_path) else {
        return Vec::new();
    };

    parse_lrc(&contents)
}

pub fn parse_lrc(contents: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    let mut global_offset_us = 0_i64;

    for raw_line in contents.lines() {
        let mut remainder = raw_line.trim();
        let mut timestamps = Vec::new();

        while let Some(stripped) = remainder.strip_prefix('[') {
            let Some(end) = stripped.find(']') else {
                break;
            };

            let tag = &stripped[..end];
            let after = &stripped[end + 1..];

            if let Some(offset_ms) = tag
                .strip_prefix("offset:")
                .and_then(|value| value.trim().parse::<i64>().ok())
            {
                global_offset_us = offset_ms.saturating_mul(1_000);
                remainder = after;
            } else if let Some(timestamp) = parse_timestamp(tag) {
                timestamps.push(timestamp);
                remainder = after;
            } else {
                remainder = after;
            }
        }

        let text = remainder.trim();
        if text.is_empty() {
            continue;
        }

        for timestamp_us in timestamps {
            lines.push(LyricLine {
                timestamp_us: timestamp_us.saturating_add(global_offset_us),
                text: text.to_string(),
            });
        }
    }

    lines.sort_by_key(|line| line.timestamp_us);
    lines
}

pub fn plain_to_lrc(contents: &str, duration_seconds: u64) -> String {
    let lines = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    if lines.is_empty() {
        return String::new();
    }

    let duration_us = duration_seconds
        .max(lines.len() as u64 * 4)
        .saturating_mul(1_000_000);
    let step_us = duration_us / lines.len() as u64;

    lines
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let timestamp_us = step_us.saturating_mul(index as u64);
            let minutes = timestamp_us / 60_000_000;
            let seconds = (timestamp_us % 60_000_000) / 1_000_000;
            let centiseconds = (timestamp_us % 1_000_000) / 10_000;
            format!("[{minutes:02}:{seconds:02}.{centiseconds:02}]{text}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn active_index(lines: &[LyricLine], timestamp_us: i64) -> Option<usize> {
    let next = lines.partition_point(|line| line.timestamp_us <= timestamp_us);
    next.checked_sub(1)
}

fn parse_timestamp(tag: &str) -> Option<i64> {
    let (minutes, seconds) = tag.split_once(':')?;
    let minutes: f64 = minutes.parse().ok()?;
    let seconds: f64 = seconds.parse().ok()?;
    Some(((minutes * 60.0 + seconds) * 1_000_000.0) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_lrc() {
        let result = parse_lrc("[00:10.50]Hello\n[01:02]World");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].timestamp_us, 10_500_000);
        assert_eq!(result[1].timestamp_us, 62_000_000);
    }

    #[test]
    fn applies_global_lrc_offset() {
        let result = parse_lrc("[offset:500]\n[00:01.00]Hello");
        assert_eq!(result[0].timestamp_us, 1_500_000);
    }

    #[test]
    fn converts_plain_lyrics_to_timed_lrc() {
        let result = plain_to_lrc("First\nSecond\nThird", 12);
        assert!(result.contains("[00:00.00]First"));
        assert!(result.contains("[00:04.00]Second"));
        assert!(result.contains("[00:08.00]Third"));
    }

    #[test]
    fn finds_active_line_with_binary_search() {
        let lines = parse_lrc("[00:01]One\n[00:05]Two\n[00:10]Three");
        assert_eq!(active_index(&lines, 500_000), None);
        assert_eq!(active_index(&lines, 1_000_000), Some(0));
        assert_eq!(active_index(&lines, 7_500_000), Some(1));
        assert_eq!(active_index(&lines, 20_000_000), Some(2));
    }
}
