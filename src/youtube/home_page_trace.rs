use super::YouTubeHomePage;
use std::{
    collections::hash_map::DefaultHasher,
    env,
    hash::{Hash, Hasher},
};

fn enabled() -> bool {
    matches!(
        env::var("NOCKY_YOUTUBE_HOME_RUST_TRACE")
            .unwrap_or_default()
            .trim(),
        "1" | "true" | "TRUE" | "yes" | "YES"
    )
}

fn page_hash(page: &YouTubeHomePage) -> u64 {
    let mut hasher = DefaultHasher::new();
    page.selected_chip_params.hash(&mut hasher);
    page.continuation.hash(&mut hasher);
    page.chips.len().hash(&mut hasher);

    for section in &page.sections {
        section.id.hash(&mut hasher);
        section.title.hash(&mut hasher);
        section.layout.hash(&mut hasher);
        section.items.len().hash(&mut hasher);

        for item in section.items.iter().take(12) {
            item.result_type.hash(&mut hasher);
            item.title.hash(&mut hasher);
            item.artist.hash(&mut hasher);
            item.video_id.hash(&mut hasher);
            item.browse_id.hash(&mut hasher);
            item.thumbnail_url.hash(&mut hasher);
            item.cover_path.hash(&mut hasher);
        }
    }

    hasher.finish()
}

fn section_titles(page: &YouTubeHomePage) -> String {
    page.sections
        .iter()
        .take(12)
        .map(|section| section.title.as_str())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn first_item(page: &YouTubeHomePage) -> String {
    page.sections
        .iter()
        .flat_map(|section| section.items.iter())
        .next()
        .map(|item| {
            format!(
                "{}:{}:{}:{}",
                item.result_type, item.title, item.artist, item.video_id
            )
        })
        .unwrap_or_default()
}

pub(crate) fn trace_youtube_home_note<T: std::fmt::Display>(
    phase: &str,
    request_id: T,
    append: bool,
    note: &str,
) {
    if !enabled() {
        return;
    }

    eprintln!(
        "[YT_HOME_RUST_TRACE] phase=\"{}\" request_id={} append={} note=\"{}\"",
        phase,
        request_id,
        append,
        note.replace('"', "'")
    );
}

pub(crate) fn trace_youtube_home_page<T: std::fmt::Display>(
    phase: &str,
    request_id: T,
    append: bool,
    page: &YouTubeHomePage,
    note: &str,
) {
    if !enabled() {
        return;
    }

    eprintln!(
        "[YT_HOME_RUST_TRACE] phase=\"{}\" request_id={} append={} page_hash=\"{:016x}\" sections={} continuation=\"{}\" selected_chip_params=\"{}\" chips={} first_item=\"{}\" section_titles=\"{}\" note=\"{}\"",
        phase,
        request_id,
        append,
        page_hash(page),
        page.sections.len(),
        page.continuation,
        page.selected_chip_params,
        page.chips.len(),
        first_item(page).replace('"', "'"),
        section_titles(page).replace('"', "'"),
        note.replace('"', "'")
    );
}
