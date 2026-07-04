#[path = "../src/youtube/home_v3.rs"]
mod home_v3;
#[path = "../src/youtube/home_v3_action.rs"]
mod home_v3_action;

use home_v3::HomeV3Item;
use home_v3_action::{item_action, HomeV3ItemAction};

#[test]
fn playable_home_item_resolves_to_play_action() {
    let item = HomeV3Item {
        video_id: " video-id ".to_string(),
        browse_id: "browse-id".to_string(),
        ..HomeV3Item::default()
    };

    assert_eq!(
        item_action(&item),
        HomeV3ItemAction::Play {
            video_id: "video-id".to_string(),
        }
    );
}

#[test]
fn browse_home_item_resolves_to_navigation_action() {
    let item = HomeV3Item {
        browse_id: " MPREb_album ".to_string(),
        params: " params ".to_string(),
        ..HomeV3Item::default()
    };

    assert_eq!(
        item_action(&item),
        HomeV3ItemAction::Browse {
            browse_id: "MPREb_album".to_string(),
            params: "params".to_string(),
        }
    );
}

#[test]
fn incomplete_home_item_is_non_destructive() {
    let item = HomeV3Item::default();

    assert_eq!(item_action(&item), HomeV3ItemAction::None);
}
