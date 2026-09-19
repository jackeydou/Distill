//! Human-readable output. `--json` bypasses this module entirely.

use distill_core::index::{NoteHit, RecallResult, Stats, SyncReport, TagCount};
use distill_core::ops::{DoctorReport, InitReport, SaveResult};

pub fn init(r: &InitReport) -> String {
    let vault_state = if r.created_vault {
        "created"
    } else {
        "joined existing vault"
    };
    format!(
        "Vault: {} ({vault_state})\nConfig: {}\nIndex: {} ({} file(s) indexed)\nWeb UI port: {}\n\nNext: install the Distill plugin in Codex or Claude Code, then run `distill doctor`.",
        r.vault.display(),
        r.config_file.display(),
        r.index_file.display(),
        r.indexed.added,
        r.ui_port,
    )
}

pub fn saved(r: &SaveResult) -> String {
    let topic = if r.topic_created {
        format!("new topic {}", r.topic_id)
    } else {
        format!("topic {}", r.topic_id)
    };
    let mut out = format!(
        "Saved note {} ({topic}, asked {} time(s))\n{}\nTags: {}",
        r.note_id,
        r.ask_count,
        r.path.display(),
        r.tags.join(", ")
    );
    if !r.tags_created.is_empty() {
        out.push_str(&format!(" (new: {})", r.tags_created.join(", ")));
    }
    out.push_str(&format!("\n{}", r.url));
    out
}

pub fn recall(r: &RecallResult) -> String {
    if r.topics.is_empty() {
        return "Nothing similar asked before.".into();
    }
    let mut out = String::new();
    for topic in &r.topics {
        out.push_str(&format!(
            "● {} — asked {} time(s)  [topic {}]\n",
            label_or(&topic.label, &topic.topic_id),
            topic.ask_count,
            topic.topic_id
        ));
        for note in &topic.notes {
            out.push_str(&format!(
                "  {}  {}  [{}]\n    {}\n",
                date(&note.created),
                note.title,
                note.id,
                first_line(&note.conclusion)
            ));
            for a in &note.annotations {
                out.push_str(&format!("    ✎ {}\n", first_line(&a.body)));
            }
        }
    }
    out.push_str(&format!("\nTags: {}", tag_list(&r.tags)));
    out
}

pub fn hits(hits: &[NoteHit]) -> String {
    if hits.is_empty() {
        return "No notes found.".into();
    }
    hits.iter()
        .map(|h| {
            format!(
                "{}  {}  [{}]  {}",
                date(&h.created),
                h.title,
                h.tags.join(", "),
                h.id
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn stats(s: &Stats) -> String {
    let mut out = format!(
        "{} note(s) in {} topic(s); {} topic(s) asked more than once; {} annotation(s)\n",
        s.notes, s.topics, s.repeated_topics, s.annotations
    );
    if !s.top_topics.is_empty() {
        out.push_str("\nMost asked:\n");
        for t in s.top_topics.iter().take(10) {
            let mark = if t.annotated { "" } else { "  (no annotation)" };
            out.push_str(&format!(
                "  {:>3}×  {}  last {}{mark}\n",
                t.ask_count,
                label_or(&t.label, &t.topic_id),
                date(&t.last_asked)
            ));
        }
    }
    if !s.tags.is_empty() {
        out.push_str(&format!("\nTags: {}\n", tag_list(&s.tags)));
    }
    if !s.by_week.is_empty() {
        out.push_str("\nBy week:\n");
        for w in s.by_week.iter().rev().take(8) {
            out.push_str(&format!("  {}  {}\n", w.week, w.notes));
        }
    }
    if s.conflicts + s.invalid_files > 0 {
        out.push_str(&format!(
            "\n{} conflict(s), {} unreadable file(s): run `distill doctor`.\n",
            s.conflicts, s.invalid_files
        ));
    }
    out.trim_end().to_string()
}

pub fn tags(tags: &[TagCount]) -> String {
    if tags.is_empty() {
        return "No tags yet.".into();
    }
    tags.iter()
        .map(|t| format!("{:>4}  {}", t.notes, t.tag))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn sync(r: &SyncReport) -> String {
    let mut out = format!("Indexed {} file(s).", r.added + r.updated + r.unchanged);
    if !r.invalid.is_empty() {
        out.push_str(&format!(
            "\n{} file(s) could not be read: {}",
            r.invalid.len(),
            r.invalid.join(", ")
        ));
    }
    out
}

pub fn doctor(r: &DoctorReport) -> String {
    let mut out = format!(
        "Config: {}\nVault: {} ({})\nIndex: {}\nWeb UI port: {}\nBinary: {}\n",
        r.config_file.display(),
        r.vault.display(),
        r.vault_id,
        r.index_file.display(),
        r.ui_port,
        r.bin_path
            .as_ref()
            .map_or("not recorded".into(), |p| p.display().to_string()),
    );
    if r.ok {
        out.push_str("\nAll good.");
        return out;
    }
    out.push('\n');
    for p in &r.problems {
        out.push_str(&format!("✗ {p}\n"));
    }
    for c in &r.conflicts {
        out.push_str(&format!("  conflict ({} {}):\n", c.kind, c.id));
        for path in &c.paths {
            out.push_str(&format!("    {path}\n"));
        }
    }
    for f in &r.invalid_files {
        out.push_str(&format!("  unreadable: {} — {}\n", f.path, f.error));
    }
    out.trim_end().to_string()
}

fn tag_list(tags: &[TagCount]) -> String {
    tags.iter()
        .map(|t| format!("{} ({})", t.tag, t.notes))
        .collect::<Vec<_>>()
        .join(", ")
}

fn label_or<'a>(label: &'a str, id: &'a str) -> &'a str {
    if label.is_empty() { id } else { label }
}

fn date(rfc3339: &str) -> &str {
    rfc3339.get(..10).unwrap_or(rfc3339)
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}
