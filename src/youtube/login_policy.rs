use std::net::IpAddr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NavigationDisposition {
    Allow,
    OpenExternal,
    Block,
}

const EMBEDDED_HOSTS: &[&str] = &[
    "accounts.google.com",
    "consent.google.com",
    "consent.youtube.com",
    "music.youtube.com",
    "myaccount.google.com",
    "www.youtube.com",
];

const EXTERNAL_HOSTS: &[&str] = &["policies.google.com", "support.google.com"];

fn https_host(uri: &str) -> Option<String> {
    let rest = uri.trim().strip_prefix("https://")?;
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .filter(|value| !value.is_empty())?;
    if authority.contains('@') || authority.starts_with('[') || authority.ends_with(']') {
        return None;
    }

    let mut host = authority;
    if let Some((candidate, port)) = authority.rsplit_once(':') {
        if candidate.contains(':') || port != "443" {
            return None;
        }
        host = candidate;
    }

    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || host.parse::<IpAddr>().is_ok()
        || host.split('.').any(|label| {
            label.is_empty()
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return None;
    }
    Some(host)
}

pub(crate) fn navigation_disposition(uri: &str) -> NavigationDisposition {
    let Some(host) = https_host(uri) else {
        return NavigationDisposition::Block;
    };
    if EMBEDDED_HOSTS.contains(&host.as_str()) {
        NavigationDisposition::Allow
    } else if EXTERNAL_HOSTS.contains(&host.as_str()) {
        NavigationDisposition::OpenExternal
    } else {
        NavigationDisposition::Block
    }
}

pub(crate) fn is_youtube_music_uri(uri: &str) -> bool {
    https_host(uri).as_deref() == Some("music.youtube.com")
}

#[cfg(test)]
mod tests {
    use super::{is_youtube_music_uri, navigation_disposition, NavigationDisposition};

    #[test]
    fn allows_only_exact_audited_https_hosts() {
        assert_eq!(
            navigation_disposition("https://accounts.google.com/v3/signin"),
            NavigationDisposition::Allow
        );
        assert_eq!(
            navigation_disposition("https://music.youtube.com/"),
            NavigationDisposition::Allow
        );
        assert_eq!(
            navigation_disposition("https://support.google.com/youtubemusic"),
            NavigationDisposition::OpenExternal
        );
    }

    #[test]
    fn blocks_lookalikes_credentials_ips_and_non_https_urls() {
        for uri in [
            "http://accounts.google.com/",
            "https://accounts.google.com.evil.example/",
            "https://user@accounts.google.com/",
            "https://127.0.0.1/",
            "https://[::1]/",
            "https://music.youtube.com:8443/",
            "data:text/html,hello",
            "javascript:alert(1)",
            "file:///tmp/session",
        ] {
            assert_eq!(
                navigation_disposition(uri),
                NavigationDisposition::Block,
                "{uri}"
            );
        }
    }

    #[test]
    fn recognizes_only_the_exact_youtube_music_origin() {
        assert!(is_youtube_music_uri("https://music.youtube.com/"));
        assert!(is_youtube_music_uri("https://music.youtube.com/library"));
        assert!(!is_youtube_music_uri("https://www.youtube.com/"));
        assert!(!is_youtube_music_uri(
            "https://music.youtube.com.evil.example/"
        ));
    }
}
