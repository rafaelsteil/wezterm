//! Search hits, quick-select match scan, and hint labels (wezterm-gui overlay).
//!
//! The 049 command-palette `Picker` lists for Search / QuickSelect / PaneSelect
//! stay in `picker.rs` behind [`PICKER_SEARCH_QUICKSELECT_PANESELECT`].

use regex::Regex;
use wezterm_term::{Line, StableRowIndex};

/// When true, Search / QuickSelect / PaneSelect use the 049 searchable lists.
/// Default off: vim-style search bar, in-pane quick-select labels, pane letter badges.
pub const PICKER_SEARCH_QUICKSELECT_PANESELECT: bool = false;

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub y: StableRowIndex,
    pub x: usize,
    pub len: usize,
}

#[derive(Clone, Debug)]
pub struct HintMatch {
    pub label: String,
    pub y: StableRowIndex,
    pub x: usize,
    pub text: String,
}

pub fn find_in_lines(
    first: StableRowIndex,
    lines: &[Line],
    query: &str,
    case_sensitive: bool,
) -> Vec<SearchHit> {
    if query.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let text = line.as_str();
        let hay = text.as_ref();
        let mut search_from = 0;
        while let Some(off) = find_sub(hay, query, search_from, case_sensitive) {
            let x = hay[..off].chars().count();
            let len = query.chars().count().max(1);
            hits.push(SearchHit {
                y: first + i as isize,
                x,
                len,
            });
            search_from = off + query.len().max(1);
            if hits.len() >= 4000 {
                return hits;
            }
        }
    }
    hits
}

fn find_sub(hay: &str, needle: &str, from: usize, case_sensitive: bool) -> Option<usize> {
    if from > hay.len() {
        return None;
    }
    let rest = &hay[from..];
    if case_sensitive {
        rest.find(needle).map(|i| from + i)
    } else {
        let h = rest.to_lowercase();
        let n = needle.to_lowercase();
        h.find(&n).map(|i| from + i)
    }
}

/// wezterm-gui `overlay/quickselect.rs` PATTERNS, compiled once.
fn qs_regexes() -> Vec<Regex> {
    const PATTERNS: [&str; 14] = [
        r"\[[^]]*\]\(([^)]+)\)",
        r"(?:https?://|git@|git://|ssh://|ftp://|file://)\S+",
        r"--- a/(\S+)",
        r"\+\+\+ b/(\S+)",
        r"sha256:([0-9a-f]{64})",
        r"(?:[.\w\-@~]+)?(?:/+[.\w\-@]+)+",
        r"#[0-9a-fA-F]{6}",
        r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
        r"Qm[0-9a-zA-Z]{44}",
        r"[0-9a-f]{7,40}",
        r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}",
        r"[A-f0-9:]+:+[A-f0-9:]+[%\w\d]+",
        r"0x[0-9a-fA-F]+",
        r"[0-9]{4,}",
    ];
    PATTERNS
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect()
}

pub fn quick_select_matches(first: StableRowIndex, lines: &[Line], alphabet: &str) -> Vec<HintMatch> {
    let regexes = qs_regexes();
    let mut spans: Vec<(StableRowIndex, usize, String)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let text = line.as_str();
        let hay = text.as_ref();
        let y = first + i as isize;
        for re in &regexes {
            for cap in re.captures_iter(hay) {
                let Some(m) = cap.get(1).or_else(|| cap.get(0)) else {
                    continue;
                };
                let x = hay[..m.start()].chars().count();
                let s = m.as_str().to_string();
                if s.len() < 4 {
                    continue;
                }
                if spans.iter().any(|(sy, sx, st)| *sy == y && *sx == x && *st == s) {
                    continue;
                }
                spans.push((y, x, s));
            }
        }
    }
    spans.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let labels = compute_labels_for_alphabet(alphabet, spans.len());
    spans
        .into_iter()
        .zip(labels)
        .map(|((y, x, text), label)| HintMatch { label, y, x, text })
        .collect()
}

/// From wezterm-gui `overlay/quickselect.rs` (tmux-thumbs MIT).
pub fn compute_labels_for_alphabet(alphabet: &str, num_matches: usize) -> Vec<String> {
    if num_matches == 0 {
        return Vec::new();
    }
    let alphabet: Vec<String> = alphabet.chars().map(|c| c.to_lowercase().to_string()).collect();
    if alphabet.is_empty() {
        return (0..num_matches).map(|i| i.to_string()).collect();
    }
    let mut primary = alphabet.clone();
    let mut secondary = vec![];
    loop {
        if primary.len() + secondary.len() >= num_matches {
            break;
        }
        let Some(prefix) = primary.pop() else {
            break;
        };
        let need = num_matches - primary.len() - secondary.len();
        let prefixed: Vec<String> = alphabet
            .iter()
            .take(need)
            .map(|s| format!("{prefix}{s}"))
            .collect();
        secondary.splice(0..0, prefixed);
    }
    primary.truncate(primary.len().min(num_matches.saturating_sub(secondary.len())));
    let mut out = primary;
    out.extend(secondary);
    out.truncate(num_matches);
    out
}

pub fn alphabet() -> String {
    let cfg = config::configuration();
    if cfg.quick_select_alphabet.is_empty() {
        "asdfqwerzxcvjklmiuopghtybn".into()
    } else {
        cfg.quick_select_alphabet.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_single_chars_first() {
        let l = compute_labels_for_alphabet("asdf", 3);
        assert_eq!(l, vec!["a", "s", "d"]);
    }

    #[test]
    fn find_case() {
        // Line::from_text is test-unfriendly; string helper is enough.
        assert_eq!(find_sub("NTUSER.DAT", "NTUSER", 0, true), Some(0));
        assert_eq!(find_sub("NTUSER.DAT", "ntuser", 0, true), None);
        assert_eq!(find_sub("NTUSER.DAT", "ntuser", 0, false), Some(0));
    }
}
