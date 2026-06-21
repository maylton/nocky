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

    for raw_line in contents.lines() {
        let mut remainder = raw_line.trim();
        let mut timestamps = Vec::new();

        while let Some(stripped) = remainder.strip_prefix('[') {
            let Some(end) = stripped.find(']') else {
                break;
            };

            let tag = &stripped[..end];
            let after = &stripped[end + 1..];

            if let Some(timestamp) = parse_timestamp(tag) {
                timestamps.push(timestamp);
                remainder = after;
            } else {
                break;
            }
        }

        let text = remainder.trim();
        if text.is_empty() {
            continue;
        }

        for timestamp_us in timestamps {
            lines.push(LyricLine {
                timestamp_us,
                text: text.to_string(),
            });
        }
    }

    lines.sort_by_key(|line| line.timestamp_us);
    lines
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
}
