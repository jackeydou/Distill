use std::io::Write;
use std::path::Path;

use crate::error::{Error, IoContext, Result};

/// Writes through a temp file in the same directory and renames it into place, so sync
/// tools and other processes never see a half-written file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().ok_or_else(|| Error::InvalidFile {
        path: path.to_path_buf(),
        reason: "path has no parent directory".into(),
    })?;
    std::fs::create_dir_all(dir).at(dir)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".distill-tmp-")
        .tempfile_in(dir)
        .at(dir)?;
    tmp.write_all(bytes).at(tmp.path())?;
    tmp.as_file().sync_all().at(tmp.path())?;
    tmp.persist(path).map_err(|e| Error::Io {
        path: path.to_path_buf(),
        source: e.error,
    })?;
    Ok(())
}

/// Splits `---\n<yaml>\n---\n<body>`. Returns `None` when the file has no frontmatter.
pub fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            let yaml = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return Some((yaml, body));
        }
        offset += line.len();
    }
    None
}

pub fn join_frontmatter(yaml: &str, body: &str) -> String {
    let mut out = String::with_capacity(yaml.len() + body.len() + 16);
    out.push_str("---\n");
    out.push_str(yaml);
    if !yaml.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("---\n");
    out.push_str(body);
    out
}

/// Current local time as RFC 3339 with offset, e.g. `2026-09-13T22:48:00-07:00`.
pub fn now_rfc3339() -> String {
    jiff::Zoned::now()
        .strftime("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

pub fn content_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_round_trip() {
        let text = join_frontmatter("id: a\n", "# Title\n\nbody\n");
        let (yaml, body) = split_frontmatter(&text).expect("has frontmatter");
        assert_eq!(yaml, "id: a\n");
        assert_eq!(body, "# Title\n\nbody\n");
    }

    #[test]
    fn frontmatter_missing() {
        assert!(split_frontmatter("# no frontmatter").is_none());
        assert!(split_frontmatter("---\nid: a\nno closing fence").is_none());
    }
}
