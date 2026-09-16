//! Secret redaction applied to every prose field before it reaches the vault, because the
//! vault is usually synced to a cloud drive.

use std::sync::LazyLock;

use regex::Regex;

struct Rule {
    kind: &'static str,
    re: Regex,
    /// Capture group holding the secret; 0 replaces the whole match.
    group: usize,
}

#[allow(clippy::unwrap_used)] // Patterns are constants; a typo fails every test.
static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    let rule = |kind, pattern: &str, group| Rule {
        kind,
        re: Regex::new(pattern).unwrap(),
        group,
    };
    vec![
        rule(
            "private-key",
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----",
            0,
        ),
        rule("api-key", r"\bsk-[A-Za-z0-9_-]{20,}", 0),
        rule(
            "github-token",
            r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,})",
            0,
        ),
        rule("aws-key", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b", 0),
        rule("slack-token", r"\bxox[abprs]-[A-Za-z0-9-]{10,}", 0),
        rule("google-key", r"\bAIza[0-9A-Za-z_-]{35}", 0),
        rule(
            "jwt",
            r"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
            0,
        ),
        rule("bearer", r"(?i)\bbearer\s+([A-Za-z0-9._~+/-]{20,}=*)", 1),
        rule(
            "secret",
            r#"(?i)\b(?:api[_-]?key|secret|token|passw(?:or)?d)\b["']?\s*[:=]\s*["']?([^\s"',;]{8,})"#,
            1,
        ),
    ]
});

#[allow(clippy::unwrap_used)]
static CANDIDATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9+/=_-]{32,}").unwrap());

/// Hex tops out at 4 bits per char, so 4.3 leaves hashes and UUIDs alone while catching
/// base64-style random tokens.
const ENTROPY_THRESHOLD: f64 = 4.3;

pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    for rule in RULES.iter() {
        out = replace_group(&rule.re, &out, rule.group, rule.kind);
    }
    CANDIDATE
        .replace_all(&out, |caps: &regex::Captures| {
            let token = &caps[0];
            if looks_random(token) {
                marker("high-entropy")
            } else {
                token.to_string()
            }
        })
        .into_owned()
}

fn replace_group(re: &Regex, text: &str, group: usize, kind: &str) -> String {
    re.replace_all(text, |caps: &regex::Captures| {
        let whole = &caps[0];
        match caps.get(group) {
            Some(m) if group > 0 => {
                let start = m.start() - caps.get(0).map_or(0, |w| w.start());
                format!(
                    "{}{}{}",
                    &whole[..start],
                    marker(kind),
                    &whole[start + m.len()..]
                )
            }
            _ => marker(kind),
        }
    })
    .into_owned()
}

fn marker(kind: &str) -> String {
    format!("[REDACTED:{kind}]")
}

fn looks_random(token: &str) -> bool {
    let has_digit = token.bytes().any(|b| b.is_ascii_digit());
    let has_upper = token.bytes().any(|b| b.is_ascii_uppercase());
    let has_lower = token.bytes().any(|b| b.is_ascii_lowercase());
    has_digit && has_upper && has_lower && shannon_entropy(token) > ENTROPY_THRESHOLD
}

fn shannon_entropy(s: &str) -> f64 {
    let mut counts = [0usize; 256];
    for b in s.bytes() {
        counts[b as usize] += 1;
    }
    let len = s.len() as f64;
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / len;
            -p * p.log2()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_known_formats() {
        let cases = [
            ("key sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123", "api-key"),
            ("ghp_abcdefghijklmnopqrstuvwxyz0123456789", "github-token"),
            ("AKIAIOSFODNN7EXAMPLE", "aws-key"),
            ("xoxb-1234567890-abcdefghij", "slack-token"),
        ];
        for (input, kind) in cases {
            let out = redact(input);
            assert!(
                out.contains(&format!("[REDACTED:{kind}]")),
                "{input} -> {out}"
            );
        }
    }

    #[test]
    fn redacts_value_but_keeps_key_name() {
        let out = redact("export API_KEY=\"s3cr3t-value-123\"");
        assert!(out.contains("API_KEY"), "{out}");
        assert!(!out.contains("s3cr3t-value-123"), "{out}");
    }

    #[test]
    fn redacts_bearer_token() {
        let out = redact("Authorization: Bearer abcdefghijklmnopqrstuvwxyz012345");
        assert_eq!(out, "Authorization: Bearer [REDACTED:bearer]");
    }

    #[test]
    fn redacts_private_key_block() {
        let key = "-----BEGIN RSA PRIVATE KEY-----\nMIIEow\n-----END RSA PRIVATE KEY-----";
        assert_eq!(redact(key), "[REDACTED:private-key]");
    }

    #[test]
    fn redacts_random_token() {
        let out = redact("token Zx9Qm2Lp7Rt4Vw8Ky3Nb6Hc1Fd5Gs0Ja2Qe7Wr");
        assert!(out.contains("[REDACTED:"), "{out}");
    }

    #[test]
    fn keeps_ordinary_text() {
        let text = concat!(
            "commit 3f2a9c1e8b7d6a5f4e3d2c1b0a9f8e7d6c5b4a3f, ",
            "session 01a0d530-42ae-7731-8a1d-b4e07d9b837d, ",
            "path /Users/me/Library/Application_Support/Distill, ",
            "中文内容不受影响"
        );
        assert_eq!(redact(text), text);
    }
}
