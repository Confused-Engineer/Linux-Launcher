//! Starting apps and revealing them in Explorer.
//!
//! On Windows everything goes through `ShellExecuteExW`, the same API Explorer
//! uses, so a single code path handles executables, shortcuts (`.lnk`),
//! scripts, documents, folders, URLs and URIs such as `ms-settings:`, and
//! supports "Run as administrator" via the `runas` verb. No `cmd.exe` is
//! involved, so arguments are not subject to shell re-parsing.

use crate::config::AppEntry;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum LaunchError {
    EmptyTarget,
    /// The user dismissed the UAC prompt; not really an error.
    Cancelled,
    Failed(String),
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTarget => write!(f, "no target set"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Failed(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for LaunchError {}

/// A fully resolved launch request (environment variables expanded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub target: String,
    pub args: String,
    pub working_dir: Option<PathBuf>,
    pub elevated: bool,
}

impl LaunchSpec {
    /// Resolves an [`AppEntry`]. When no working directory is configured and
    /// the target is an existing file, its parent folder is used.
    pub fn from_app(app: &AppEntry, force_elevated: bool) -> Self {
        let target = expand_env(app.target.trim());
        let args = expand_env(app.args.trim());
        let working_dir = match app.working_dir.trim() {
            "" => Path::new(&target)
                .is_file()
                .then(|| Path::new(&target).parent().map(Path::to_path_buf))
                .flatten(),
            dir => Some(PathBuf::from(expand_env(dir))),
        };
        Self {
            target,
            args,
            working_dir,
            elevated: app.run_as_admin || force_elevated,
        }
    }
}

/// Launches an app.
pub fn launch(app: &AppEntry, force_elevated: bool) -> Result<(), LaunchError> {
    let spec = LaunchSpec::from_app(app, force_elevated);
    if spec.target.is_empty() {
        return Err(LaunchError::EmptyTarget);
    }
    log::info!("launching {spec:?}");
    platform::launch(&spec)
}

/// If `target` (after env expansion) is an existing file or folder, returns it.
pub fn local_path(target: &str) -> Option<PathBuf> {
    let path = PathBuf::from(expand_env(target.trim()));
    path.exists().then_some(path)
}

/// Opens Explorer with the target selected (or opens the folder itself).
pub fn reveal(target: &str) -> Result<(), LaunchError> {
    let path = local_path(target)
        .ok_or_else(|| LaunchError::Failed(format!("\"{target}\" is not a file or folder")))?;
    platform::reveal(&path)
}

/// Opens a file or folder with its default handler (e.g. the config folder).
pub fn open_path(path: &Path) -> Result<(), LaunchError> {
    platform::launch(&LaunchSpec {
        target: path.to_string_lossy().into_owned(),
        args: String::new(),
        working_dir: None,
        elevated: false,
    })
}

/// Expands `%VAR%` references using the process environment, the way
/// `cmd.exe` does. Unknown variables and lone `%` are left untouched.
pub fn expand_env(input: &str) -> String {
    expand_with(input, |name| std::env::var(name).ok())
}

fn expand_with(input: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 && !after[..end].contains(char::is_whitespace) => {
                let name = &after[..end];
                match lookup(name) {
                    Some(value) => out.push_str(&value),
                    None => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(windows)]
mod platform {
    use super::{LaunchError, LaunchSpec};
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use windows::Win32::Foundation::ERROR_CANCELLED;
    use windows::Win32::UI::Shell::{
        SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, PCWSTR, w};

    pub fn launch(spec: &LaunchSpec) -> Result<(), LaunchError> {
        let file = HSTRING::from(spec.target.as_str());
        let params = HSTRING::from(spec.args.as_str());
        let dir = spec.working_dir.as_deref().map(HSTRING::from);

        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
            lpVerb: if spec.elevated {
                w!("runas")
            } else {
                PCWSTR::null()
            },
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: if spec.args.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(params.as_ptr())
            },
            lpDirectory: dir.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };

        // SAFETY: `info` is fully initialised and every string pointer it holds
        // refers to an HSTRING that outlives this call.
        match unsafe { ShellExecuteExW(&mut info) } {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => Err(LaunchError::Cancelled),
            Err(e) => Err(LaunchError::Failed(e.message().trim().to_owned())),
        }
    }

    pub fn reveal(path: &Path) -> Result<(), LaunchError> {
        // `/select,"path"` must reach explorer unescaped, hence `raw_arg`.
        std::process::Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn()
            .map(drop)
            .map_err(|e| LaunchError::Failed(e.to_string()))
    }
}

#[cfg(not(windows))]
mod platform {
    //! Best-effort fallback so the crate builds and runs on other platforms.
    use super::{LaunchError, LaunchSpec};
    use std::path::Path;
    use std::process::Command;

    const OPENER: &str = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };

    pub fn launch(spec: &LaunchSpec) -> Result<(), LaunchError> {
        let mut cmd = if spec.args.is_empty() {
            let mut c = Command::new(OPENER);
            c.arg(&spec.target);
            c
        } else {
            let mut c = Command::new(&spec.target);
            c.args(spec.args.split_whitespace());
            c
        };
        if let Some(dir) = &spec.working_dir {
            cmd.current_dir(dir);
        }
        cmd.spawn()
            .map(drop)
            .map_err(|e| LaunchError::Failed(e.to_string()))
    }

    pub fn reveal(path: &Path) -> Result<(), LaunchError> {
        let dir = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        Command::new(OPENER)
            .arg(dir)
            .spawn()
            .map(drop)
            .map_err(|e| LaunchError::Failed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(name: &str) -> Option<String> {
        match name.to_ascii_uppercase().as_str() {
            "HOME" => Some(r"C:\Users\me".into()),
            "APP" => Some("tool".into()),
            _ => None,
        }
    }

    #[test]
    fn expands_known_variables() {
        assert_eq!(expand_with(r"%HOME%\x.exe", lookup), r"C:\Users\me\x.exe");
        assert_eq!(expand_with("%home%-%APP%", lookup), r"C:\Users\me-tool");
    }

    #[test]
    fn leaves_unknown_and_stray_percent_signs() {
        assert_eq!(expand_with("%NOPE%", lookup), "%NOPE%");
        assert_eq!(expand_with("100%", lookup), "100%");
        assert_eq!(expand_with("50% off 20%", lookup), "50% off 20%");
        assert_eq!(expand_with("%%", lookup), "%%");
        assert_eq!(expand_with("a%HOME%b%", lookup), r"aC:\Users\meb%");
    }

    #[test]
    fn url_encoded_text_is_untouched() {
        let url = "https://example.com/?q=a%20b%2Fc";
        assert_eq!(expand_with(url, lookup), url);
    }

    #[test]
    fn spec_defaults_working_dir_to_target_folder() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("tool.exe");
        std::fs::write(&exe, b"").unwrap();
        let spec = LaunchSpec::from_app(&AppEntry::new("t", exe.to_string_lossy()), false);
        assert_eq!(spec.working_dir.as_deref(), Some(dir.path()));
        assert!(!spec.elevated);

        let url = AppEntry {
            run_as_admin: true,
            ..AppEntry::new("u", "https://example.com")
        };
        let spec = LaunchSpec::from_app(&url, false);
        assert_eq!(spec.working_dir, None);
        assert!(spec.elevated);
    }
}
