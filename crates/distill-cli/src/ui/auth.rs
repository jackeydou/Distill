//! Who may call the web UI's API (spec D7). Any page the user has open can send requests to
//! a loopback port, so every API call needs the session secret: the browser holds it as a
//! cookie, the CLI sends it as a bearer token. A browser gets the cookie by redeeming a
//! one-time token that `distill ui` puts in the URL it opens.
//!
//! The secret and pending tokens live in `<data dir>/ui/`, readable only by the user, so
//! whichever process started the server, `distill ui` in a terminal can authorize a browser.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};

pub const COOKIE: &str = "distill_session";
/// A token in a URL that was never opened stops working after this long.
const TOKEN_TTL: Duration = Duration::from_secs(5 * 60);
/// Browsers cap cookie lifetime at 400 days.
const COOKIE_MAX_AGE_SECS: u64 = 400 * 24 * 60 * 60;

#[derive(Clone)]
pub struct Auth {
    secret: String,
    tokens_dir: PathBuf,
}

impl Auth {
    /// Reads the secret in `dir`, creating it on first use. Two processes starting at once
    /// both end up with the same secret: it is created with a hard link, which fails when
    /// the file already exists.
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.join("secret");
        if !path.exists() {
            let tmp = dir.join(format!(".secret-{}", std::process::id()));
            write_private(&tmp, random_hex()?.as_bytes())?;
            let linked = std::fs::hard_link(&tmp, &path);
            std::fs::remove_file(&tmp).with_context(|| format!("removing {}", tmp.display()))?;
            match linked {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => {
                    return Err(e).with_context(|| format!("creating {}", path.display()));
                }
            }
        }
        let secret = std::fs::read_to_string(&path)
            .with_context(|| format!("reading the UI secret {}", path.display()))?
            .trim()
            .to_string();
        anyhow::ensure!(
            secret.len() >= 32,
            "the UI secret {} is too short; delete it and run `distill ui` again",
            path.display()
        );
        Ok(Self {
            secret,
            tokens_dir: dir.join("tokens"),
        })
    }

    pub fn secret(&self) -> &str {
        &self.secret
    }

    /// Compared through BLAKE3 hashes, whose equality check runs in constant time.
    pub fn check(&self, candidate: &str) -> bool {
        blake3::hash(candidate.as_bytes()) == blake3::hash(self.secret.as_bytes())
    }

    /// A token for one browser to redeem at `/auth` within five minutes.
    pub fn issue_token(&self) -> Result<String> {
        let token = random_hex()?;
        write_private(&self.token_file(&token), b"")?;
        Ok(token)
    }

    /// True once per issued token, and only while it is fresh.
    pub fn redeem(&self, token: &str) -> Result<bool> {
        if token.len() != 64 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(false);
        }
        let path = self.token_file(token);
        let Ok(meta) = std::fs::metadata(&path) else {
            return Ok(false);
        };
        std::fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
        let age = meta
            .modified()
            .ok()
            .and_then(|m| SystemTime::now().duration_since(m).ok());
        Ok(age.is_some_and(|a| a < TOKEN_TTL))
    }

    pub fn set_cookie(&self) -> String {
        format!(
            "{COOKIE}={}; Path=/; HttpOnly; SameSite=Strict; Max-Age={COOKIE_MAX_AGE_SECS}",
            self.secret
        )
    }

    /// Token files are named by the token's hash, so listing the directory reveals nothing
    /// that can be redeemed.
    fn token_file(&self, token: &str) -> PathBuf {
        self.tokens_dir
            .join(blake3::hash(token.as_bytes()).to_hex().as_str())
    }
}

/// The value of `name` in a `Cookie` header.
pub fn cookie_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.split(';').find_map(|pair| {
        let (k, v) = pair.trim().split_once('=')?;
        (k == name).then_some(v)
    })
}

fn random_hex() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("no system randomness: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut file = opts
        .open(path)
        .with_context(|| format!("creating {}", path.display()))?;
    file.write_all(bytes)
        .with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn secret_is_shared_and_tokens_redeem_once() {
        let dir = tempfile::tempdir().unwrap();
        let a = Auth::load_or_create(dir.path()).unwrap();
        let b = Auth::load_or_create(dir.path()).unwrap();
        assert_eq!(a.secret(), b.secret());
        assert!(a.check(b.secret()));
        assert!(!a.check("guess"));

        let token = a.issue_token().unwrap();
        assert!(b.redeem(&token).unwrap());
        assert!(!b.redeem(&token).unwrap());
        assert!(!b.redeem("../secret").unwrap());
    }

    #[test]
    fn reads_cookie_values() {
        let header = "theme=dark; distill_session=abc; other=1";
        assert_eq!(cookie_value(header, COOKIE), Some("abc"));
        assert_eq!(cookie_value("x=1", COOKIE), None);
    }
}
