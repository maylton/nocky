//! User feedback helpers for `AppController`.

use super::AppController;
use crate::app::media::redact_stream_url;

impl AppController {
    pub(crate) fn show_toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_use_markup(false);
        self.toast_overlay.add_toast(toast);
    }

    pub(crate) fn show_error(&self, message: &str) {
        if let Some(detail) = message.strip_prefix("__NOCKY_STREAM_RECOVERY_FAILED__") {
            self.youtube_recovery_in_progress.set(false);
            self.youtube_recovery_resume_us.set(0);
            self.youtube_recovery_was_playing.set(false);
            eprintln!(
                "Nocky stream recovery failed: {}",
                redact_stream_url(detail)
            );
            let friendly =
                "Não foi possível renovar o stream desta faixa. Tente reproduzi-la novamente.";
            self.album.set_text(friendly);
            self.show_toast(friendly);
            return;
        }

        eprintln!("Nocky error: {}", redact_stream_url(message));
        self.album.set_text(&format!("Error: {message}"));
        self.show_toast(message);
    }
}
