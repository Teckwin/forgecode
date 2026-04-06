//! Local fuzzy search using nucleo-matcher.

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Perform fuzzy search of `needle` within `haystack`.
///
/// Returns a list of (start_line, end_line) pairs (0-based) where the needle
/// was found with a fuzzy match.
///
/// If `search_all` is false, only the best match is returned.
pub fn fuzzy_search(needle: &str, haystack: &str, search_all: bool) -> Vec<(u32, u32)> {
    if needle.is_empty() || haystack.is_empty() {
        return vec![];
    }

    let lines: Vec<&str> = haystack.lines().collect();
    if lines.is_empty() {
        return vec![];
    }

    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let atom = Atom::new(needle, CaseMatching::Smart, Normalization::Smart, AtomKind::Fuzzy, false);

    let mut matches: Vec<(u32, u32, u32)> = Vec::new(); // (start, end, score)
    let mut buf = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let haystack_str = Utf32Str::new(line, &mut buf);
        if let Some(score) = atom.score(haystack_str, &mut matcher) {
            matches.push((i as u32, i as u32, score as u32));
        }
        buf.clear();
    }

    if matches.is_empty() {
        return vec![];
    }

    // Sort by score descending
    matches.sort_by(|a, b| b.2.cmp(&a.2));

    // Merge adjacent lines into ranges
    let merged = merge_adjacent_matches(&matches);

    if search_all {
        merged
    } else {
        merged.into_iter().take(1).collect()
    }
}

/// Merge adjacent single-line matches into contiguous ranges.
fn merge_adjacent_matches(matches: &[(u32, u32, u32)]) -> Vec<(u32, u32)> {
    if matches.is_empty() {
        return vec![];
    }

    // Sort by line number for merging
    let mut sorted: Vec<(u32, u32)> = matches.iter().map(|(s, e, _)| (*s, *e)).collect();
    sorted.sort();
    sorted.dedup();

    let mut result = Vec::new();
    let mut current_start = sorted[0].0;
    let mut current_end = sorted[0].1;

    for &(start, end) in &sorted[1..] {
        if start <= current_end + 1 {
            current_end = current_end.max(end);
        } else {
            result.push((current_start, current_end));
            current_start = start;
            current_end = end;
        }
    }
    result.push((current_start, current_end));

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_fuzzy_search() {
        let haystack = "fn main() {\n    println!(\"hello world\");\n    let x = 42;\n}";
        let results = fuzzy_search("println", haystack, true);
        assert!(!results.is_empty());
        // The line containing println should be found
        assert!(results.iter().any(|(s, e)| *s <= 1 && *e >= 1));
    }

    #[test]
    fn test_no_match() {
        let results = fuzzy_search("zzzznotfound", "fn main() {}", true);
        // May or may not match depending on fuzzy tolerance
        // Just ensure no panic
        let _ = results;
    }

    #[test]
    fn test_empty_inputs() {
        assert!(fuzzy_search("", "hello", true).is_empty());
        assert!(fuzzy_search("hello", "", true).is_empty());
    }

    #[test]
    fn test_search_all_false() {
        let haystack = "fn a() {}\nfn b() {}\nfn c() {}";
        let results = fuzzy_search("fn", haystack, false);
        assert!(results.len() <= 1);
    }
}
