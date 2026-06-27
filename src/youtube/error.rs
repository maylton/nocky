use crate::config::AppLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum YouTubePlaybackErrorKind {
    RegionBlocked,
    PrivateOrRemoved,
    AgeRestricted,
    AuthenticationRequired,
    Unavailable,
    TemporaryNetwork,
    Unknown,
}

impl YouTubePlaybackErrorKind {
    pub(crate) fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::RegionBlocked
                | Self::PrivateOrRemoved
                | Self::AgeRestricted
                | Self::AuthenticationRequired
                | Self::Unavailable
        )
    }

    pub(crate) fn message(self, language: AppLanguage) -> &'static str {
        match language {
            AppLanguage::Portuguese => match self {
                Self::RegionBlocked => {
                    "Esta faixa não está disponível na sua região. Pulando para a próxima."
                }
                Self::PrivateOrRemoved => {
                    "Esta faixa foi removida ou tornou-se privada. Pulando para a próxima."
                }
                Self::AgeRestricted => {
                    "Esta faixa possui restrição de idade e não pode ser reproduzida nesta sessão."
                }
                Self::AuthenticationRequired => {
                    "O YouTube exige uma sessão autenticada para reproduzir esta faixa."
                }
                Self::Unavailable => {
                    "Esta faixa não está disponível no YouTube. Pulando para a próxima."
                }
                Self::TemporaryNetwork => {
                    "A reprodução online foi interrompida. Verifique a conexão e tente novamente."
                }
                Self::Unknown => "Não foi possível reproduzir esta faixa.",
            },
            AppLanguage::English => match self {
                Self::RegionBlocked => {
                    "This track is not available in your region. Skipping to the next track."
                }
                Self::PrivateOrRemoved => {
                    "This track was removed or made private. Skipping to the next track."
                }
                Self::AgeRestricted => {
                    "This track is age-restricted and cannot be played with the current session."
                }
                Self::AuthenticationRequired => {
                    "YouTube requires an authenticated session to play this track."
                }
                Self::Unavailable => {
                    "This track is unavailable on YouTube. Skipping to the next track."
                }
                Self::TemporaryNetwork => {
                    "Online playback was interrupted. Check your connection and try again."
                }
                Self::Unknown => "This track could not be played.",
            },
            AppLanguage::Spanish => match self {
                Self::RegionBlocked => {
                    "Esta canción no está disponible en tu región. Pasando a la siguiente."
                }
                Self::PrivateOrRemoved => {
                    "Esta canción fue eliminada o ahora es privada. Pasando a la siguiente."
                }
                Self::AgeRestricted => {
                    "Esta canción tiene restricción de edad y no puede reproducirse con la sesión actual."
                }
                Self::AuthenticationRequired => {
                    "YouTube requiere una sesión autenticada para reproducir esta canción."
                }
                Self::Unavailable => {
                    "Esta canción no está disponible en YouTube. Pasando a la siguiente."
                }
                Self::TemporaryNetwork => {
                    "La reproducción en línea fue interrumpida. Comprueba la conexión e inténtalo de nuevo."
                }
                Self::Unknown => "No se pudo reproducir esta canción.",
            },
        }
    }
}

pub(crate) fn classify_youtube_playback_error(message: &str) -> YouTubePlaybackErrorKind {
    let lower = message.to_ascii_lowercase();

    if contains_any(
        &lower,
        &[
            "not available in your country",
            "not available in your region",
            "blocked in your country",
            "geo restricted",
            "geo-restricted",
            "region blocked",
        ],
    ) {
        return YouTubePlaybackErrorKind::RegionBlocked;
    }

    if contains_any(
        &lower,
        &[
            "private video",
            "video is private",
            "removed by the uploader",
            "video has been removed",
            "this video has been removed",
            "account associated with this video has been terminated",
        ],
    ) {
        return YouTubePlaybackErrorKind::PrivateOrRemoved;
    }

    if contains_any(
        &lower,
        &[
            "sign in to confirm your age",
            "age-restricted",
            "age restricted",
            "confirm your age",
            "inappropriate for some users",
        ],
    ) {
        return YouTubePlaybackErrorKind::AgeRestricted;
    }

    if contains_any(
        &lower,
        &[
            "sign in to confirm you're not a bot",
            "sign in to confirm you’re not a bot",
            "login required",
            "authentication required",
            "members-only",
            "members only",
            "this video is available to this channel's members",
        ],
    ) {
        return YouTubePlaybackErrorKind::AuthenticationRequired;
    }

    if contains_any(
        &lower,
        &[
            "video unavailable",
            "this video is unavailable",
            "content unavailable",
            "requested format is not available",
            "no video formats found",
            "premieres in",
            "not available",
        ],
    ) {
        return YouTubePlaybackErrorKind::Unavailable;
    }

    if contains_any(
        &lower,
        &[
            "connection reset",
            "connection timed out",
            "timed out",
            "temporary failure",
            "network is unreachable",
            "host is unreachable",
            "could not connect",
            "internal data stream error",
            "souphttpsrc",
            "googlevideo.com",
        ],
    ) {
        return YouTubePlaybackErrorKind::TemporaryNetwork;
    }

    YouTubePlaybackErrorKind::Unknown
}

fn contains_any(message: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| message.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_region_blocking() {
        assert_eq!(
            classify_youtube_playback_error("ERROR: This video is not available in your country"),
            YouTubePlaybackErrorKind::RegionBlocked
        );
    }

    #[test]
    fn classifies_private_or_removed_tracks() {
        assert_eq!(
            classify_youtube_playback_error("ERROR: Private video"),
            YouTubePlaybackErrorKind::PrivateOrRemoved
        );
        assert_eq!(
            classify_youtube_playback_error("This video has been removed by the uploader"),
            YouTubePlaybackErrorKind::PrivateOrRemoved
        );
    }

    #[test]
    fn classifies_age_restriction_before_generic_authentication() {
        assert_eq!(
            classify_youtube_playback_error("Sign in to confirm your age"),
            YouTubePlaybackErrorKind::AgeRestricted
        );
    }

    #[test]
    fn classifies_authentication_requirements() {
        assert_eq!(
            classify_youtube_playback_error("Sign in to confirm you're not a bot"),
            YouTubePlaybackErrorKind::AuthenticationRequired
        );
    }

    #[test]
    fn classifies_generic_unavailability() {
        assert_eq!(
            classify_youtube_playback_error("ERROR: Video unavailable"),
            YouTubePlaybackErrorKind::Unavailable
        );
    }

    #[test]
    fn classifies_temporary_network_failures() {
        assert_eq!(
            classify_youtube_playback_error("gstsouphttpsrc: connection timed out"),
            YouTubePlaybackErrorKind::TemporaryNetwork
        );
    }

    #[test]
    fn only_permanent_failures_are_terminal() {
        assert!(YouTubePlaybackErrorKind::RegionBlocked.is_terminal());
        assert!(YouTubePlaybackErrorKind::Unavailable.is_terminal());
        assert!(!YouTubePlaybackErrorKind::TemporaryNetwork.is_terminal());
        assert!(!YouTubePlaybackErrorKind::Unknown.is_terminal());
    }
}
