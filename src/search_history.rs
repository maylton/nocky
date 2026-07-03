use crate::search_text::normalize_search_text;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const SEARCH_HISTORY_VERSION: u32 = 1;
const SEARCH_HISTORY_LIMIT: usize = 20;
const MIN_QUERY_CHARACTERS: usize = 2;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct StoredSearchHistory {
    version: u32,
    queries: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SearchHistory {
    queries: Vec<String>,
}

impl SearchHistory {
    pub fn load() -> Self {
        let Ok(raw) = fs::read_to_string(search_history_path()) else {
            return Self::default();
        };
        let Ok(stored) = serde_json::from_str::<StoredSearchHistory>(&raw) else {
            return Self::default();
        };
        if stored.version != SEARCH_HISTORY_VERSION {
            return Self::default();
        }

        let mut history = Self::default();
        for query in stored.queries.into_iter().rev() {
            history.record_in_memory(&query);
        }
        history
    }

    pub fn queries(&self) -> &[String] {
        &self.queries
    }

    pub fn record(&mut self, raw_query: &str) -> bool {
        let changed = self.record_in_memory(raw_query);
        if changed {
            self.save();
        }
        changed
    }

    pub fn remove(&mut self, raw_query: &str) -> bool {
        let changed = self.remove_in_memory(raw_query);
        if changed {
            self.save();
        }
        changed
    }

    pub fn clear(&mut self) -> bool {
        let changed = self.clear_in_memory();
        if changed {
            self.save();
        }
        changed
    }

    fn remove_in_memory(&mut self, raw_query: &str) -> bool {
        let key = normalize_search_text(raw_query);
        let Some(index) = self
            .queries
            .iter()
            .position(|query| normalize_search_text(query) == key)
        else {
            return false;
        };
        self.queries.remove(index);
        true
    }

    fn clear_in_memory(&mut self) -> bool {
        if self.queries.is_empty() {
            return false;
        }
        self.queries.clear();
        true
    }

    fn record_in_memory(&mut self, raw_query: &str) -> bool {
        let query = normalized_display_query(raw_query);
        if query.chars().count() < MIN_QUERY_CHARACTERS {
            return false;
        }
        let key = normalize_search_text(&query);
        let previous = self
            .queries
            .iter()
            .position(|candidate| normalize_search_text(candidate) == key);
        let unchanged_at_front = previous == Some(0)
            && self
                .queries
                .first()
                .is_some_and(|candidate| candidate == &query);
        if unchanged_at_front {
            return false;
        }
        if let Some(index) = previous {
            self.queries.remove(index);
        }
        self.queries.insert(0, query);
        self.queries.truncate(SEARCH_HISTORY_LIMIT);
        true
    }

    fn save(&self) {
        if let Err(error) = save_search_history(&self.queries) {
            eprintln!("Could not save recent searches: {error}");
        }
    }
}

fn normalized_display_query(raw_query: &str) -> String {
    raw_query.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn save_search_history(queries: &[String]) -> Result<(), String> {
    let path = search_history_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create search history folder: {error}"))?;
    }
    let payload = StoredSearchHistory {
        version: SEARCH_HISTORY_VERSION,
        queries: queries.to_vec(),
    };
    let serialized = serde_json::to_vec(&payload)
        .map_err(|error| format!("could not serialize search history: {error}"))?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, serialized)
        .map_err(|error| format!("could not write search history: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("could not replace search history: {error}"))?;
    Ok(())
}

fn search_history_path() -> PathBuf {
    gtk::glib::user_data_dir()
        .join("nocky")
        .join("search-history.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_normalizes_deduplicates_and_moves_queries_to_the_front() {
        let mut history = SearchHistory::default();
        assert!(history.record_in_memory("  Daft   Punk  "));
        assert!(history.record_in_memory("Muse"));
        assert!(history.record_in_memory("DAFT PUNK"));

        assert_eq!(history.queries, vec!["DAFT PUNK", "Muse"]);
        assert!(!history.record_in_memory("DAFT PUNK"));
    }

    #[test]
    fn history_ignores_single_character_queries_and_keeps_a_bounded_mru_list() {
        let mut history = SearchHistory::default();
        assert!(!history.record_in_memory("x"));
        for index in 0..25 {
            assert!(history.record_in_memory(&format!("query {index}")));
        }

        assert_eq!(history.queries.len(), SEARCH_HISTORY_LIMIT);
        assert_eq!(
            history.queries.first().map(String::as_str),
            Some("query 24")
        );
        assert_eq!(history.queries.last().map(String::as_str), Some("query 5"));
    }

    #[test]
    fn remove_and_clear_update_the_in_memory_history() {
        let mut history = SearchHistory::default();
        history.record_in_memory("Massive Attack");
        history.record_in_memory("Portishead");

        assert!(history.remove_in_memory("massive attack"));
        assert_eq!(history.queries, vec!["Portishead"]);
        assert!(!history.remove_in_memory("missing"));
        assert!(history.clear_in_memory());
        assert!(history.queries.is_empty());
        assert!(!history.clear_in_memory());
    }
}
