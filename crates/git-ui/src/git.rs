//! Thin wrapper around the `git` command line.
//!
//! Shelling out to `git` (instead of linking libgit2) means we inherit the
//! user's existing credential helper. On Windows that is Git Credential
//! Manager, so pushing to GitHub works with the same login the CLI already
//! uses, including 2FA and SSH keys.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Keeps a console window from flashing up for every `git` call.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

impl CommandOutput {
    /// stdout and stderr joined, for display in the log pane.
    pub fn text(&self) -> String {
        let mut s = String::new();
        if !self.stdout.trim().is_empty() {
            s.push_str(self.stdout.trim_end());
        }
        if !self.stderr.trim().is_empty() {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(self.stderr.trim_end());
        }
        s
    }
}

fn base_command(cwd: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(cwd);
    // Never block on an interactive terminal prompt: a GUI app has no console
    // to answer it, so git would hang forever. Credential *helpers*, which pop
    // their own window, are unaffected by this.
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    // Stable, parseable English output.
    cmd.env("LC_ALL", "C");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Run a git command inside `cwd` and capture everything it printed.
pub fn run(cwd: &Path, args: &[&str]) -> CommandOutput {
    let pretty = format!("git {}", args.join(" "));
    let mut cmd = base_command(cwd);
    cmd.args(args);
    match cmd.output() {
        Ok(out) => CommandOutput {
            command: pretty,
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            success: out.status.success(),
        },
        Err(err) => CommandOutput {
            command: pretty,
            stdout: String::new(),
            stderr: format!("could not launch git: {err}\nIs git installed and on PATH?"),
            success: false,
        },
    }
}

fn capture(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = run(cwd, args);
    if !out.success {
        return None;
    }
    let s = out.stdout.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Two-character porcelain code: index status, then worktree status.
    pub code: String,
    pub path: String,
    pub staged: bool,
}

impl FileEntry {
    pub fn label(&self) -> &'static str {
        match self.code.as_str() {
            "??" => "new",
            "UU" => "conflict",
            c if c.starts_with('R') => "renamed",
            c if c.starts_with('A') => "added",
            c if c.starts_with('D') || c.ends_with('D') => "deleted",
            _ => "modified",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RepoStatus {
    pub root: PathBuf,
    pub branch: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub remote_url: Option<String>,
    pub files: Vec<FileEntry>,
    pub recent_commits: Vec<String>,
    pub branches: Vec<String>,
    pub detached: bool,
}

impl RepoStatus {
    pub fn is_dirty(&self) -> bool {
        !self.files.is_empty()
    }
}

/// Read everything the UI needs in one go. `path` may be any directory inside
/// the working tree.
pub fn status(path: &Path) -> Result<RepoStatus, String> {
    if !path.is_dir() {
        return Err(format!("{} is not a folder", path.display()));
    }

    let top = run(path, &["rev-parse", "--show-toplevel"]);
    if !top.success {
        return Err(if top.stderr.trim().is_empty() {
            format!("{} is not a git repository", path.display())
        } else {
            top.text()
        });
    }
    let root = PathBuf::from(top.stdout.trim());

    let out = run(
        &root,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
        ],
    );
    if !out.success {
        return Err(out.text());
    }

    let mut st = RepoStatus {
        root: root.clone(),
        branch: "(unknown)".to_string(),
        ..Default::default()
    };

    for line in out.stdout.lines() {
        if let Some(header) = line.strip_prefix("# ") {
            let mut parts = header.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some("branch.head"), Some(name)) => {
                    if name == "(detached)" {
                        st.detached = true;
                        st.branch = "detached HEAD".to_string();
                    } else {
                        st.branch = name.to_string();
                    }
                }
                (Some("branch.upstream"), Some(name)) => st.upstream = Some(name.to_string()),
                (Some("branch.ab"), Some(ahead)) => {
                    st.ahead = ahead.trim_start_matches('+').parse().unwrap_or(0);
                    if let Some(behind) = parts.next() {
                        st.behind = behind.trim_start_matches('-').parse().unwrap_or(0);
                    }
                }
                _ => {}
            }
            continue;
        }

        let mut parts = line.splitn(2, ' ');
        let kind = parts.next().unwrap_or_default();
        let rest = parts.next().unwrap_or_default();
        match kind {
            // 1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>
            "1" => {
                let fields: Vec<&str> = rest.splitn(8, ' ').collect();
                if fields.len() == 8 {
                    st.files.push(FileEntry {
                        code: fields[0].to_string(),
                        path: fields[7].to_string(),
                        staged: !fields[0].starts_with('.'),
                    });
                }
            }
            // 2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <score> <path>\t<origPath>
            "2" => {
                let fields: Vec<&str> = rest.splitn(9, ' ').collect();
                if fields.len() == 9 {
                    let path = fields[8].split('\t').next().unwrap_or(fields[8]);
                    st.files.push(FileEntry {
                        code: fields[0].to_string(),
                        path: path.to_string(),
                        staged: !fields[0].starts_with('.'),
                    });
                }
            }
            // u <XY> ... <path>   (unmerged / conflict)
            "u" => {
                if let Some(path) = rest.rsplit(' ').next() {
                    st.files.push(FileEntry {
                        code: "UU".to_string(),
                        path: path.to_string(),
                        staged: false,
                    });
                }
            }
            "?" => st.files.push(FileEntry {
                code: "??".to_string(),
                path: rest.to_string(),
                staged: false,
            }),
            _ => {}
        }
    }

    st.remote_url = capture(&root, &["remote", "get-url", "origin"]);
    st.recent_commits = capture(
        &root,
        &[
            "log",
            "-n",
            "12",
            "--date=short",
            "--pretty=format:%h  %ad  %an  %s",
        ],
    )
    .map(|s| s.lines().map(str::to_string).collect())
    .unwrap_or_default();
    st.branches = capture(
        &root,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )
    .map(|s| s.lines().map(str::to_string).collect())
    .unwrap_or_default();

    st.files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(st)
}

/// Turn an `origin` URL into something a browser can open.
pub fn web_url(remote: &str) -> Option<String> {
    let remote = remote.trim().trim_end_matches(".git");
    let candidate = if let Some(rest) = remote.strip_prefix("git@") {
        format!("https://{}", rest.replacen(':', "/", 1))
    } else if let Some(rest) = remote.strip_prefix("ssh://git@") {
        format!("https://{rest}")
    } else if remote.starts_with("http://") || remote.starts_with("https://") {
        remote.to_string()
    } else {
        return None;
    };

    is_safe_web_url(&candidate).then_some(candidate)
}

/// Whether a URL is safe to hand to the system's URL handler.
///
/// The remote URL is not trusted input: it comes from `.git/config`, which
/// arrives with any repository that gets cloned. A conservative allow-list is
/// used rather than escaping, because the rules for quoting a string safely
/// differ between every handler it might reach.
///
/// Anything with credentials (`@`), a query string, a fragment, whitespace,
/// control characters or shell metacharacters is rejected. A URL for browsing
/// a repository needs none of those.
fn is_safe_web_url(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };

    if rest.is_empty() || url.len() > 2048 {
        return false;
    }
    // A host has to come before the first '/'.
    let host = rest.split('/').next().unwrap_or_default();
    if host.is_empty() || !host.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        return false;
    }

    rest.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/' | '~' | '+' | ':'))
}

/// Open a URL in the default browser.
///
/// Goes through `webbrowser`, which calls the platform's handler API directly.
/// Nothing is passed through a shell, so the URL cannot be read as a command.
pub fn open_url(url: &str) {
    if is_safe_web_url(url) {
        let _ = webbrowser::open(url);
    }
}

/// Reveal a folder in the system file manager.
///
/// The path is passed as a single process argument, never through a shell.
pub fn open_folder(path: &Path) {
    if !path.is_dir() {
        return;
    }
    #[cfg(windows)]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("explorer.exe");
        c.creation_flags(CREATE_NO_WINDOW);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = Command::new("xdg-open");

    let _ = cmd.arg(path).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_shell_metacharacters_in_remote() {
        // The `&` would end the command and start a new one if this string
        // ever reached a shell.
        assert_eq!(
            web_url("https://github.com/o/r&calc.exe"),
            None,
            "a remote URL carrying a command separator must not be opened"
        );
        assert_eq!(web_url("https://github.com/o/r\nwhoami"), None);
        assert_eq!(web_url("https://github.com/o/r | whoami"), None);
        assert_eq!(web_url("https://user:pw@evil.example/o/r"), None);
    }

    #[test]
    fn accepts_ordinary_remotes() {
        assert_eq!(
            web_url("https://github.com/Henrytorybot/Action-Based-Generation.git"),
            Some("https://github.com/Henrytorybot/Action-Based-Generation".to_string())
        );
        assert_eq!(
            web_url("git@github.com:Henrytorybot/Action-Based-Generation.git"),
            Some("https://github.com/Henrytorybot/Action-Based-Generation".to_string())
        );
        assert_eq!(
            web_url("ssh://git@github.com/o/r"),
            Some("https://github.com/o/r".to_string())
        );
    }

    #[test]
    fn rejects_non_http_schemes() {
        assert_eq!(web_url("file:///C:/Windows"), None);
        assert_eq!(web_url("javascript:alert(1)"), None);
        assert_eq!(web_url(""), None);
    }
}
