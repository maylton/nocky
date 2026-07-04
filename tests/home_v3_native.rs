#[path = "../src/youtube/home_v3.rs"]
mod home_v3;
#[path = "../src/youtube/home_v3_adapter.rs"]
mod home_v3_adapter;
#[path = "../src/youtube/home_v3_native.rs"]
mod home_v3_native;

use home_v3_native::{
    parse_native_home_v3_helper_response, parse_native_home_v3_payload, HomeV3NativeParseError,
};

#[test]
fn parses_native_home_v3_payload_into_source_contract() {
    let payload = r#"{
        "version": 3,
        "selected_chip_params": "chip-selected",
        "chips": [
            { "title": "Energize", "params": "chip-selected" }
        ],
        "sections": [
            {
                "title": "Quick picks",
                "layout": "carousel",
                "items": [
                    {
                        "result_type": "song",
                        "title": "Song",
                        "subtitle": "Artist",
                        "video_id": "video-id",
                        "browse_id": "",
                        "album": "Album",
                        "artist": "Artist",
                        "playlist_kind": "",
                        "params": "",
                        "duration_seconds": 123,
                        "thumbnail_url": "https://example.invalid/cover.jpg",
                        "cover_path": "/tmp/cover"
                    }
                ]
            }
        ],
        "continuation": "next-page"
    }"#;

    let page = parse_native_home_v3_payload(payload).expect("valid native Home V3 payload");

    assert_eq!(page.selected_chip_params, "chip-selected");
    assert_eq!(page.chips[0].title, "Energize");
    assert_eq!(page.chips[0].params, "chip-selected");
    assert_eq!(page.sections[0].title, "Quick picks");
    assert_eq!(page.sections[0].layout, "carousel");

    let item = &page.sections[0].items[0];
    assert_eq!(item.result_type, "song");
    assert_eq!(item.title, "Song");
    assert_eq!(item.subtitle, "Artist");
    assert_eq!(item.video_id, "video-id");
    assert_eq!(item.album, "Album");
    assert_eq!(item.artist, "Artist");
    assert_eq!(item.duration_seconds, 123);
    assert_eq!(item.thumbnail_url, "https://example.invalid/cover.jpg");
    assert_eq!(item.cover_path, "/tmp/cover");
    assert_eq!(page.continuation, "next-page");
}

#[test]
fn accepts_empty_native_home_v3_payload_without_legacy_fallback() {
    let payload = r#"{
        "version": 3,
        "selected_chip_params": "",
        "chips": [],
        "sections": [],
        "continuation": ""
    }"#;

    let page = parse_native_home_v3_payload(payload).expect("empty native Home V3 is valid");

    assert!(page.chips.is_empty());
    assert!(page.sections.is_empty());
    assert!(page.continuation.is_empty());
}

#[test]
fn rejects_unsupported_native_home_v3_payload_version() {
    let error = parse_native_home_v3_payload(r#"{ "version": 2 }"#)
        .expect_err("version 2 must not be accepted as Home V3");

    assert_eq!(error, HomeV3NativeParseError::UnsupportedVersion(2));
}

#[test]
fn rejects_invalid_native_home_v3_json() {
    let error = parse_native_home_v3_payload("{not-json").expect_err("invalid JSON must fail");

    assert!(matches!(error, HomeV3NativeParseError::Json(_)));
}

#[test]
fn parses_native_home_v3_helper_response_wrapper() {
    let payload = br#"{
        "ok": true,
        "result": {
            "chips": [
                { "title": "All", "params": "" }
            ],
            "sections": [
                {
                    "title": "Quick picks",
                    "layout": "carousel",
                    "items": [
                        { "title": "Song", "video_id": "video-id" }
                    ]
                }
            ],
            "continuation": "next-page",
            "selected_chip_params": ""
        },
        "error": null
    }"#;

    let page = parse_native_home_v3_helper_response(payload).expect("valid helper response");

    assert_eq!(page.chips[0].title, "All");
    assert_eq!(page.sections[0].title, "Quick picks");
    assert_eq!(page.sections[0].items[0].video_id, "video-id");
    assert_eq!(page.continuation, "next-page");
}

#[test]
fn rejects_failed_native_home_v3_helper_response() {
    let error = parse_native_home_v3_helper_response(
        br#"{ "ok": false, "result": null, "error": "boom" }"#,
    )
    .expect_err("failed helper response must fail");

    assert_eq!(error, HomeV3NativeParseError::Helper("boom".to_string()));
}

#[test]
fn rejects_native_home_v3_helper_response_without_result() {
    let error =
        parse_native_home_v3_helper_response(br#"{ "ok": true, "result": null, "error": null }"#)
            .expect_err("missing helper result must fail");

    assert_eq!(error, HomeV3NativeParseError::MissingResult);
}
