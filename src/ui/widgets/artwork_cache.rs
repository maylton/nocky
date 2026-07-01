//! Shared, bounded artwork texture cache for GTK presentation surfaces.

use gtk::{gdk, gio};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
};

const ARTWORK_TEXTURE_CACHE_LIMIT: usize = 256;

#[derive(Default)]
struct ArtworkTextureCache {
    entries: HashMap<PathBuf, CachedArtworkTexture>,
    clock: u64,
}

struct CachedArtworkTexture {
    texture: gdk::Texture,
    last_used: u64,
}

thread_local! {
    static ARTWORK_TEXTURES: RefCell<ArtworkTextureCache> =
        RefCell::new(ArtworkTextureCache::default());
}

pub(crate) fn artwork_texture(path: &Path) -> Option<gdk::Texture> {
    if let Some(texture) = ARTWORK_TEXTURES.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.clock = cache.clock.wrapping_add(1);
        let now = cache.clock;
        cache.entries.get_mut(path).map(|entry| {
            entry.last_used = now;
            entry.texture.clone()
        })
    }) {
        return Some(texture);
    }

    let texture = gdk::Texture::from_file(&gio::File::for_path(path)).ok()?;
    ARTWORK_TEXTURES.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.clock = cache.clock.wrapping_add(1);
        let now = cache.clock;
        if cache.entries.len() >= ARTWORK_TEXTURE_CACHE_LIMIT {
            if let Some(oldest) = cache
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            {
                cache.entries.remove(&oldest);
            }
        }
        cache.entries.insert(
            path.to_path_buf(),
            CachedArtworkTexture {
                texture: texture.clone(),
                last_used: now,
            },
        );
    });
    Some(texture)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_has_a_bounded_capacity() {
        assert_eq!(ARTWORK_TEXTURE_CACHE_LIMIT, 256);
    }
}
