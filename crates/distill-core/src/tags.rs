//! Tag rules: agents reuse existing tags, and a new tag must be deliberate. See the spec's
//! D5 for why the save API separates `tags` from `new_tags`.

use std::collections::{HashMap, HashSet};

use crate::error::{Error, Result};

pub const MAX_TAGS_PER_NOTE: usize = 5;

/// Normalized edit similarity at or above this counts as a near-duplicate
/// (`sqlite` vs `sqlite3` scores 0.86).
const SIMILARITY_THRESHOLD: f64 = 0.8;

/// Canonical form: trimmed, lowercase, full-width folded to ASCII, runs of spaces and
/// underscores collapsed to one `-`.
pub fn normalize(raw: &str) -> Result<String> {
    let mut out = String::with_capacity(raw.len());
    let mut pending_dash = false;
    for c in raw.trim().chars().map(fold_width) {
        if c.is_whitespace() || c == '_' || c == '-' {
            pending_dash = !out.is_empty();
            continue;
        }
        if pending_dash {
            out.push('-');
            pending_dash = false;
        }
        out.extend(c.to_lowercase());
    }
    if !out.chars().any(char::is_alphanumeric) {
        return Err(Error::EmptyTag {
            tag: raw.to_string(),
        });
    }
    Ok(out)
}

fn fold_width(c: char) -> char {
    match c as u32 {
        0x3000 => ' ',
        cp @ 0xFF01..=0xFF5E => char::from_u32(cp - 0xFEE0).unwrap_or(c),
        _ => c,
    }
}

/// Existing tags a new tag is suspiciously close to. Semantic duplicates such as `db`
/// and `database` are not caught here; the agent judges those from the tag list.
pub fn similar_to<'a>(candidate: &str, existing: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let squash = |s: &str| s.replace('-', "");
    let candidate_squashed = squash(candidate);
    existing
        .into_iter()
        .filter(|e| *e != candidate)
        .filter(|e| {
            squash(e) == candidate_squashed
                || strsim::normalized_levenshtein(candidate, e) >= SIMILARITY_THRESHOLD
        })
        .map(str::to_string)
        .collect()
}

/// Follows `from -> to` alias chains to the final tag. A cycle resolves to the tag where
/// the cycle was detected, so a bad alias file cannot hang the index.
pub fn resolve_alias(tag: &str, aliases: &HashMap<String, String>) -> String {
    let mut current = tag.to_string();
    let mut seen = HashSet::new();
    while let Some(next) = aliases.get(&current) {
        if !seen.insert(current.clone()) {
            break;
        }
        current = next.clone();
    }
    current
}

/// Case-insensitive near matches for an unknown tag, for the error message.
pub fn suggestions<'a>(tag: &str, existing: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut scored: Vec<(f64, &str)> = existing
        .into_iter()
        .map(|e| (strsim::normalized_levenshtein(tag, e), e))
        .filter(|(score, e)| *score >= 0.5 || e.contains(tag) || tag.contains(*e))
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored
        .into_iter()
        .take(5)
        .map(|(_, e)| e.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes() {
        assert_eq!(normalize("  React Query ").unwrap(), "react-query");
        assert_eq!(normalize("react__query").unwrap(), "react-query");
        assert_eq!(normalize("ＳＱＬｉｔｅ").unwrap(), "sqlite");
        assert_eq!(normalize("数据库").unwrap(), "数据库");
        assert_eq!(normalize("C++").unwrap(), "c++");
        assert!(matches!(normalize(" - _ "), Err(Error::EmptyTag { .. })));
    }

    #[test]
    fn finds_near_duplicates() {
        let existing = ["sqlite", "react-query", "rust"];
        assert_eq!(similar_to("sqlite3", existing), vec!["sqlite"]);
        assert_eq!(similar_to("reactquery", existing), vec!["react-query"]);
        assert!(similar_to("go", existing).is_empty());
        assert!(similar_to("database", ["db"]).is_empty());
    }

    #[test]
    fn resolves_alias_chain_and_cycles() {
        let aliases: HashMap<String, String> = [("a", "b"), ("b", "c"), ("x", "y"), ("y", "x")]
            .into_iter()
            .map(|(f, t)| (f.to_string(), t.to_string()))
            .collect();
        assert_eq!(resolve_alias("a", &aliases), "c");
        assert_eq!(resolve_alias("z", &aliases), "z");
        let cyclic = resolve_alias("x", &aliases);
        assert!(cyclic == "x" || cyclic == "y");
    }
}
