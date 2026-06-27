//! Persistence and history helpers for `AppController`.

use super::*;

impl AppController {
    pub(crate) fn save_config(&self) {
        if let Err(error) = self.config.borrow().save() {
            eprintln!("Could not save Nocky settings: {error}");
        }
    }
}
