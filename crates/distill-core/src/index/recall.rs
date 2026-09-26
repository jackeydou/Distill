//! Ranking for recall: "have I asked something like this before?"
//!
//! A reworded question rarely shares three consecutive characters with the old one
//! ("加载扩展" vs "加载不了扩展"), so the trigram FTS index used by search misses it.
//! Recall instead scores every note in memory on CJK bigrams and whole words, weighted by
//! IDF. A personal vault holds thousands of notes, which this handles in milliseconds.
//! When the embedding model is installed, this ranking is fused with the semantic one
//! (`semantic.rs`); without it, this is all recall has.

use std::collections::{HashMap, HashSet};

/// Share of the question's total IDF weight a note must cover to count as a candidate.
/// Keeps notes that only share filler like "为什么" out of the results.
const MIN_COVERAGE: f64 = 0.25;
/// Terms found in the title or question count this much more than terms in the body.
const HEAD_WEIGHT: f64 = 2.0;

pub(super) struct Doc<'a> {
    pub id: &'a str,
    pub head: String,
    pub body: &'a str,
}

/// Note ids ranked by similarity to `question`, best first, above the coverage floor.
pub(super) fn rank(question: &str, docs: &[Doc]) -> Vec<String> {
    let query = terms(question);
    if query.is_empty() || docs.is_empty() {
        return Vec::new();
    }
    let indexed: Vec<(HashSet<String>, HashSet<String>)> = docs
        .iter()
        .map(|d| (terms(&d.head), terms(d.body)))
        .collect();
    let n = docs.len() as f64;
    let idf: HashMap<&String, f64> = query
        .iter()
        .map(|t| {
            let df = indexed
                .iter()
                .filter(|(h, b)| h.contains(t) || b.contains(t))
                .count() as f64;
            (t, (1.0 + n / (df + 1.0)).ln())
        })
        .collect();
    let total: f64 = idf.values().sum::<f64>() * HEAD_WEIGHT;
    let mut scored: Vec<(f64, &str)> = docs
        .iter()
        .zip(&indexed)
        .filter_map(|(doc, (head, body))| {
            let score: f64 = query
                .iter()
                .map(|t| {
                    let w = if head.contains(t) {
                        HEAD_WEIGHT
                    } else if body.contains(t) {
                        1.0
                    } else {
                        0.0
                    };
                    w * idf[t]
                })
                .sum();
            (score > 0.0 && score >= total * MIN_COVERAGE).then_some((score, doc.id))
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.into_iter().map(|(_, id)| id.to_string()).collect()
}

/// Reciprocal rank fusion: each list adds `1 / (K + rank)` for the ids it holds, so an id
/// ranked well by either keyword or embedding recall surfaces, and one ranked well by
/// both comes first.
pub(super) fn fuse(lists: &[&[String]]) -> Vec<String> {
    const K: f64 = 60.0;
    let mut scores: HashMap<&str, f64> = HashMap::new();
    for list in lists {
        for (rank, id) in list.iter().enumerate() {
            *scores.entry(id).or_default() += 1.0 / (K + rank as f64 + 1.0);
        }
    }
    let mut ids: Vec<(&str, f64)> = scores.into_iter().collect();
    ids.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
    ids.into_iter().map(|(id, _)| id.to_string()).collect()
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF)
}

/// Lowercased words of two or more characters, and every CJK bigram (a lone CJK
/// character counts as itself).
pub(super) fn terms(text: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut word = String::new();
    let mut cjk: Vec<char> = Vec::new();
    let flush = |word: &mut String, cjk: &mut Vec<char>, out: &mut HashSet<String>| {
        if word.chars().count() >= 2 {
            out.insert(word.to_lowercase());
        }
        word.clear();
        match cjk.len() {
            0 => {}
            1 => {
                out.insert(cjk[0].to_string());
            }
            _ => out.extend(cjk.windows(2).map(|w| w.iter().collect::<String>())),
        }
        cjk.clear();
    };
    for c in text.chars() {
        if is_cjk(c) {
            if !word.is_empty() {
                flush(&mut word, &mut Vec::new(), &mut out);
            }
            cjk.push(c);
        } else if c.is_alphanumeric() || c == '+' || c == '#' {
            if !cjk.is_empty() {
                flush(&mut String::new(), &mut cjk, &mut out);
            }
            word.push(c);
        } else {
            flush(&mut word, &mut cjk, &mut out);
        }
    }
    flush(&mut word, &mut cjk, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc<'a>(id: &'a str, head: &str, body: &'a str) -> Doc<'a> {
        Doc {
            id,
            head: head.to_string(),
            body,
        }
    }

    #[test]
    fn terms_mix_words_and_bigrams() {
        let t = terms("bun 加载扩展 ok");
        for expected in ["bun", "ok", "加载", "载扩", "扩展"] {
            assert!(t.contains(expected), "{expected} missing from {t:?}");
        }
        assert!(terms("a 字").contains("字"));
    }

    #[test]
    fn reworded_question_still_matches() {
        let docs = [
            doc(
                "A",
                "bun:sqlite 为什么加载不了扩展",
                "系统 SQLite 关闭了扩展加载",
            ),
            doc("B", "SQLite WAL 并发写入", "一写多读"),
            doc("C", "为什么 React 会重复渲染", "StrictMode"),
        ];
        assert_eq!(rank("加载扩展", &docs), vec!["A"]);
        assert_eq!(rank("sqlite 扩展为什么加载失败", &docs)[0], "A");
    }

    #[test]
    fn fusion_rewards_agreement() {
        let s = |ids: &[&str]| ids.iter().map(|i| i.to_string()).collect::<Vec<_>>();
        let keyword = s(&["A", "B"]);
        let semantic = s(&["C", "B"]);
        assert_eq!(fuse(&[&keyword, &semantic]), s(&["B", "A", "C"]));
        assert_eq!(fuse(&[&keyword, &[]]), keyword);
    }

    #[test]
    fn filler_words_alone_do_not_match() {
        let docs = [
            doc("A", "为什么 bun 加载不了扩展", ""),
            doc("B", "为什么 React 会重复渲染", ""),
            doc("C", "为什么 git rebase 会冲突", ""),
        ];
        assert!(rank("为什么天是蓝的", &docs).is_empty());
    }
}
