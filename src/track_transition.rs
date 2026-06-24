// nocky_track_metadata_artwork_transitions_v1
use gtk::{glib, prelude::*};
use std::{
    cell::Cell,
    rc::Rc,
    time::Duration,
};

const FADE_OUT_MS: u64 = 95;

#[derive(Clone)]
pub(crate) struct MetadataTransition {
    title: gtk::Label,
    artist: gtk::Label,
    album: Option<gtk::Label>,
    generation: Rc<Cell<u64>>,
}

impl MetadataTransition {
    pub(crate) fn new(
        title: &gtk::Label,
        artist: &gtk::Label,
        album: Option<&gtk::Label>,
    ) -> Self {
        title.add_css_class("track-meta-transition");
        title.add_css_class("track-meta-title");
        artist.add_css_class("track-meta-transition");
        artist.add_css_class("track-meta-artist");

        if let Some(album) = album {
            album.add_css_class("track-meta-transition");
            album.add_css_class("track-meta-album");
        }

        Self {
            title: title.clone(),
            artist: artist.clone(),
            album: album.cloned(),
            generation: Rc::new(Cell::new(0)),
        }
    }

    pub(crate) fn set(
        &self,
        title: &str,
        artist: &str,
        album: Option<&str>,
    ) {
        let title = title.to_string();
        let artist = artist.to_string();
        let album = album.map(str::to_string);

        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);

        if !adw::is_animations_enabled(&self.title) {
            self.apply(&title, &artist, album.as_deref());
            self.set_faded(false);
            return;
        }

        self.set_faded(true);

        let transition = self.clone();
        glib::timeout_add_local_once(
            Duration::from_millis(FADE_OUT_MS),
            move || {
                if transition.generation.get() != generation {
                    return;
                }

                transition.apply(&title, &artist, album.as_deref());
                transition.set_faded(false);
            },
        );
    }

    fn apply(
        &self,
        title: &str,
        artist: &str,
        album: Option<&str>,
    ) {
        self.title.set_text(title);
        self.artist.set_text(artist);

        if let (Some(label), Some(text)) = (&self.album, album) {
            label.set_text(text);
        }
    }

    fn set_faded(&self, faded: bool) {
        for label in [
            Some(&self.title),
            Some(&self.artist),
            self.album.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if faded {
                label.add_css_class("track-meta-transition-out");
            } else {
                label.remove_css_class("track-meta-transition-out");
            }
        }
    }
}
