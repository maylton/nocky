use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LikeMutationPhase {
    #[default]
    Idle,
    Pending,
    Confirmed,
    RolledBack,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LikeMutation {
    pub previous: bool,
    pub target: bool,
    pub phase: LikeMutationPhase,
    pub message: String,
}

impl LikeMutation {
    pub fn visible_value(&self) -> bool {
        match self.phase {
            LikeMutationPhase::Pending | LikeMutationPhase::Confirmed => self.target,
            LikeMutationPhase::Idle | LikeMutationPhase::RolledBack => self.previous,
        }
    }

    pub fn pending(&self) -> bool {
        self.phase == LikeMutationPhase::Pending
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LikeMutationStartError {
    MissingId,
    AlreadyPending,
    Unchanged,
}

#[derive(Clone, Debug, Default)]
pub struct LikeMutationRegistry {
    entries: HashMap<String, LikeMutation>,
}

impl LikeMutationRegistry {
    pub fn begin(
        &mut self,
        video_id: &str,
        previous: bool,
        target: bool,
    ) -> Result<&LikeMutation, LikeMutationStartError> {
        let video_id = video_id.trim();
        if video_id.is_empty() {
            return Err(LikeMutationStartError::MissingId);
        }
        if previous == target {
            return Err(LikeMutationStartError::Unchanged);
        }
        if self.entries.get(video_id).is_some_and(LikeMutation::pending) {
            return Err(LikeMutationStartError::AlreadyPending);
        }

        self.entries.insert(
            video_id.to_string(),
            LikeMutation {
                previous,
                target,
                phase: LikeMutationPhase::Pending,
                message: String::new(),
            },
        );
        Ok(self.entries.get(video_id).expect("mutation inserted"))
    }

    pub fn get(&self, video_id: &str) -> Option<&LikeMutation> {
        self.entries.get(video_id)
    }

    pub fn confirm(&mut self, video_id: &str) -> bool {
        let Some(entry) = self.entries.get_mut(video_id) else {
            return false;
        };
        if !entry.pending() {
            return false;
        }
        entry.phase = LikeMutationPhase::Confirmed;
        entry.message.clear();
        true
    }

    pub fn rollback(&mut self, video_id: &str, message: impl Into<String>) -> bool {
        let Some(entry) = self.entries.get_mut(video_id) else {
            return false;
        };
        if !entry.pending() {
            return false;
        }
        entry.phase = LikeMutationPhase::RolledBack;
        entry.message = message.into();
        true
    }

    pub fn clear_finished(&mut self, video_id: &str) -> bool {
        if self.entries.get(video_id).is_some_and(|entry| !entry.pending()) {
            self.entries.remove(video_id);
            return true;
        }
        false
    }
}
