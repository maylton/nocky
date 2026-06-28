use super::YouTubeItem;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum YouTubeItemAction {
    Ignore,
    Continue,
    Play,
    OpenPlaylist,
    OpenCollection,
    Unsupported,
}

pub(crate) fn youtube_item_action(item: &YouTubeItem) -> YouTubeItemAction {
    let result_type = item.result_type.trim().to_ascii_lowercase();

    if matches!(result_type.as_str(), "section" | "chips" | "carousel") {
        return YouTubeItemAction::Ignore;
    }

    if result_type == "continuation" {
        return if item.params.trim().is_empty() {
            YouTubeItemAction::Ignore
        } else {
            YouTubeItemAction::Continue
        };
    }

    if item.playable() {
        return YouTubeItemAction::Play;
    }

    match result_type.as_str() {
        "playlist" if !item.browse_id.trim().is_empty() => YouTubeItemAction::OpenPlaylist,
        "album" | "artist"
            if !item.browse_id.trim().is_empty() || !item.title.trim().is_empty() =>
        {
            YouTubeItemAction::OpenCollection
        }
        "" => YouTubeItemAction::Ignore,
        _ => YouTubeItemAction::Unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(result_type: &str) -> YouTubeItem {
        YouTubeItem {
            result_type: result_type.to_string(),
            title: "Example".to_string(),
            ..YouTubeItem::default()
        }
    }

    #[test]
    fn playable_episode_uses_play_action() {
        let mut episode = item("episode");
        episode.video_id = "abcdefghijk".to_string();

        assert_eq!(youtube_item_action(&episode), YouTubeItemAction::Play);
    }

    #[test]
    fn album_and_artist_open_native_collection_views() {
        let mut album = item("album");
        album.browse_id = "MPREexample".to_string();
        let mut artist = item("artist");
        artist.browse_id = "UCexample".to_string();

        assert_eq!(
            youtube_item_action(&album),
            YouTubeItemAction::OpenCollection
        );
        assert_eq!(
            youtube_item_action(&artist),
            YouTubeItemAction::OpenCollection
        );
    }

    #[test]
    fn playlist_requires_a_browse_identifier() {
        let mut playlist = item("playlist");
        assert_eq!(
            youtube_item_action(&playlist),
            YouTubeItemAction::Unsupported
        );

        playlist.browse_id = "PLexample".to_string();
        assert_eq!(
            youtube_item_action(&playlist),
            YouTubeItemAction::OpenPlaylist
        );
    }

    #[test]
    fn continuation_requires_parameters() {
        let mut continuation = item("continuation");
        assert_eq!(
            youtube_item_action(&continuation),
            YouTubeItemAction::Ignore
        );

        continuation.params = "6".to_string();
        assert_eq!(
            youtube_item_action(&continuation),
            YouTubeItemAction::Continue
        );
    }

    #[test]
    fn structural_rows_are_not_activatable() {
        assert_eq!(
            youtube_item_action(&item("section")),
            YouTubeItemAction::Ignore
        );
        assert_eq!(
            youtube_item_action(&item("chips")),
            YouTubeItemAction::Ignore
        );
        assert_eq!(
            youtube_item_action(&item("carousel")),
            YouTubeItemAction::Ignore
        );
    }

    #[test]
    fn podcast_and_unknown_collection_types_are_explicitly_unsupported() {
        assert_eq!(
            youtube_item_action(&item("podcast")),
            YouTubeItemAction::Unsupported
        );
        assert_eq!(
            youtube_item_action(&item("audiobook")),
            YouTubeItemAction::Unsupported
        );
    }
}
