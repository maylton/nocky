/// Normalizes user-visible metadata for forgiving library search.
///
/// This intentionally stays dependency-free and covers the Latin characters
/// commonly found in Portuguese, Spanish, French, German and music metadata.
pub(crate) fn normalize_search_text(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());

    for character in value.chars().flat_map(char::to_lowercase) {
        match character {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => normalized.push('a'),
            'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => normalized.push('c'),
            'ď' | 'đ' => normalized.push('d'),
            'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => normalized.push('e'),
            'ĝ' | 'ğ' | 'ġ' | 'ģ' => normalized.push('g'),
            'ĥ' | 'ħ' => normalized.push('h'),
            'í' | 'ì' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => normalized.push('i'),
            'ĵ' => normalized.push('j'),
            'ķ' => normalized.push('k'),
            'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => normalized.push('l'),
            'ñ' | 'ń' | 'ņ' | 'ň' | 'ŉ' | 'ŋ' => normalized.push('n'),
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => normalized.push('o'),
            'ŕ' | 'ŗ' | 'ř' => normalized.push('r'),
            'ś' | 'ŝ' | 'ş' | 'š' => normalized.push('s'),
            'ţ' | 'ť' | 'ŧ' => normalized.push('t'),
            'ú' | 'ù' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => {
                normalized.push('u')
            }
            'ŵ' => normalized.push('w'),
            'ý' | 'ÿ' | 'ŷ' => normalized.push('y'),
            'ź' | 'ż' | 'ž' => normalized.push('z'),
            'æ' => normalized.push_str("ae"),
            'œ' => normalized.push_str("oe"),
            'ß' => normalized.push_str("ss"),
            character if character.is_alphanumeric() => normalized.push(character),
            _ => {
                if !normalized.ends_with(' ') {
                    normalized.push(' ');
                }
            }
        }
    }

    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn search_matches(haystack: &str, normalized_query: &str) -> bool {
    if normalized_query.is_empty() {
        return true;
    }

    let haystack = normalize_search_text(haystack);
    normalized_query
        .split_whitespace()
        .all(|term| haystack.contains(term))
}

pub(crate) fn search_score(haystack: &str, normalized_query: &str) -> u8 {
    let haystack = normalize_search_text(haystack);
    if haystack == normalized_query {
        0
    } else if haystack.starts_with(normalized_query) {
        1
    } else if haystack
        .split_whitespace()
        .any(|word| word.starts_with(normalized_query))
    {
        2
    } else if normalized_query
        .split_whitespace()
        .all(|term| haystack.contains(term))
    {
        3
    } else {
        u8::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_accents_and_punctuation() {
        assert_eq!(normalize_search_text("João & Beyoncé"), "joao beyonce");
        assert_eq!(normalize_search_text("Sigur Rós"), "sigur ros");
    }

    #[test]
    fn matches_all_query_terms_in_any_position() {
        assert!(search_matches("The Fame — Lady Gaga", "lady fame"));
        assert!(!search_matches("The Fame — Lady Gaga", "lady chromatica"));
    }

    #[test]
    fn exact_and_prefix_matches_rank_first() {
        assert!(search_score("Beyoncé", "beyonce") < search_score("Best of Beyoncé", "beyonce"));
    }
    #[test]
    fn ranking_handles_accents_and_word_order_consistently() {
        let query = normalize_search_text("gaga fame");
        assert_eq!(search_score("The Fame — Lady Gaga", &query), 3);
        assert!(search_matches("The Fame — Lady Gaga", &query));

        let accented = normalize_search_text("beyonce");
        assert_eq!(search_score("Beyoncé", &accented), 0);
    }
}
