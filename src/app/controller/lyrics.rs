//! Lyrics controller methods for `AppController`.

use super::*;

impl AppController {
    pub(crate) fn rebuild_lyrics(&self, track: &Track) {
        if track.lyrics.is_empty() {
            let automatic = self.config.borrow().auto_download_lyrics;
            self.lyrics.show_state(
                "Nenhuma letra sincronizada disponível ainda",
                Some(if automatic {
                    "Automatic LRCLIB lookup is enabled. Use the menu to retry whenever needed."
                } else {
                    "Use the menu to download lyrics, or place a matching .lrc file beside the song."
                }),
                "No synchronized lyrics available yet",
                Some(if automatic {
                    "Automatic LRCLIB lookup is enabled. You can also open the Lyrics page for the full view."
                } else {
                    "Use the menu to download lyrics, or open the Lyrics page for the full view."
                }),
            );
            return;
        }

        self.lyrics.set_lines(&track.lyrics);
    }

    pub(crate) fn rebuild_youtube_lyrics(&self, lyrics: &[LyricLine]) {
        if lyrics.is_empty() {
            self.set_lyrics_message("No synchronized lyrics available for this YouTube track yet.");
            return;
        }

        self.lyrics.set_lines(lyrics);
    }

    pub(crate) fn highlight_lyric(&self, timestamp: i64) {
        self.lyrics.update_timestamp(timestamp);
    }

    pub(crate) fn set_lyrics_message(&self, message: &str) {
        self.lyrics.show_message(message, None);
    }

    pub(crate) fn request_lyrics(&self, index: usize, notify: bool, force: bool) {
        let (path, lookup) = {
            let state = self.state.borrow();
            let Some(track) = state.tracks.get(index) else {
                return;
            };
            if !force && !track.lyrics.is_empty() {
                return;
            }
            (
                track.path.clone(),
                lyrics_domain::provider::LyricsLookup {
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: track.album.clone(),
                    duration_seconds: track.duration_seconds,
                },
            )
        };

        if !self.lyrics_pending.borrow_mut().insert(path.clone()) {
            if notify {
                self.show_toast("As letras já estão sendo buscadas");
            }
            return;
        }

        if notify {
            self.show_toast("Buscando letras sincronizadas...");
        }
        let sender = self.background.sender();
        thread::spawn(move || {
            let result = lyrics_domain::provider::download_to_sidecar(&path, &lookup, force).map(
                |document| {
                    eprintln!(
                        "Lyrics loaded from {} ({})",
                        document.provider,
                        if document.synchronized {
                            "synchronized"
                        } else {
                            "plain fallback"
                        }
                    );
                },
            );
            let _ = sender.send(BackgroundMessage::LyricsDownloaded {
                path,
                result,
                notify,
            });
        });
    }

    pub(crate) fn refresh_current_lyrics(&self) {
        match self.playback_source.get() {
            PlaybackSource::Local => {
                let current = self.state.borrow().current;
                let Some(index) = current else {
                    self.show_toast("Selecione uma faixa primeiro");
                    return;
                };
                self.request_lyrics(index, true, true);
            }
            PlaybackSource::YouTube => {
                let item = self
                    .youtube_state
                    .borrow()
                    .as_ref()
                    .map(|state| state.item.clone());
                let Some(item) = item else {
                    self.show_toast("Selecione uma faixa primeiro");
                    return;
                };

                self.set_lyrics_message("Buscando novamente as letras sincronizadas…");
                self.show_toast("Buscando letras sincronizadas…");
                self.request_youtube_lyrics(&item, true);
            }
            PlaybackSource::None => {
                self.show_toast("Selecione uma faixa primeiro");
            }
        }
    }
}
