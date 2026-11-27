//! Locating, loading and saving the configuration file.
//!
//! * Installed mode (default): `%APPDATA%\LinuxLauncher\config.toml`.
//! * Portable mode: `config.toml` next to the executable, enabled when that
//!   file or an empty `portable` marker file exists beside the `.exe`.
//!
//! On first run an old `LL.ini` (next to the exe or in `~/.linuxlauncher`) is
//! migrated automatically; otherwise the bundled starter config is written.

use super::{Config, from_legacy_ini};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE_NAME: &str = "config.toml";
const PORTABLE_MARKER: &str = "portable";
const LEGACY_FILE_NAME: &str = "LL.ini";

#[derive(Debug)]
pub enum StoreError {
    Io(PathBuf, io::Error),
    Toml(PathBuf, Box<toml::de::Error>),
    Legacy(PathBuf, super::LegacyError),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(p, e) => write!(f, "{}: {e}", p.display()),
            Self::Toml(p, e) => write!(f, "{}: {e}", p.display()),
            Self::Legacy(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

impl std::error::Error for StoreError {}

/// Result of [`ConfigStore::load`].
#[derive(Debug)]
pub enum LoadOutcome {
    /// The existing config file was read successfully.
    Loaded(Config),
    /// No config existed; the starter config was created.
    Created(Config),
    /// No config existed; a legacy `LL.ini` was converted.
    Migrated { config: Config, from: PathBuf },
    /// The config exists but could not be read. It is left untouched so the
    /// user can fix it; the app should not overwrite it automatically.
    Invalid(StoreError),
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
    portable: bool,
}

impl ConfigStore {
    /// Picks the config location for this machine (portable or per-user).
    pub fn locate() -> Self {
        if let Some(exe_dir) = exe_dir() {
            let portable_cfg = exe_dir.join(CONFIG_FILE_NAME);
            if portable_cfg.exists() || exe_dir.join(PORTABLE_MARKER).exists() {
                return Self {
                    path: portable_cfg,
                    portable: true,
                };
            }
        }
        let dir = user_config_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            path: dir.join(CONFIG_FILE_NAME),
            portable: false,
        }
    }

    /// A store backed by an explicit file (used by tests).
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            portable: false,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn dir(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new("."))
    }

    pub fn is_portable(&self) -> bool {
        self.portable
    }

    /// Loads the config, migrating or creating it on first run.
    pub fn load(&self) -> LoadOutcome {
        self.load_with_legacy(&legacy_candidates())
    }

    pub fn load_with_legacy(&self, legacy: &[PathBuf]) -> LoadOutcome {
        match fs::read_to_string(&self.path) {
            Ok(text) => match Config::from_toml(&text) {
                Ok(config) => LoadOutcome::Loaded(config),
                Err(e) => LoadOutcome::Invalid(StoreError::Toml(self.path.clone(), Box::new(e))),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let (config, from) = legacy
                    .iter()
                    .find_map(|p| {
                        let text = fs::read_to_string(p).ok()?;
                        match from_legacy_ini(&text) {
                            Ok(c) => Some((c, p.clone())),
                            Err(e) => {
                                log::warn!("skipping legacy config {}: {e}", p.display());
                                None
                            }
                        }
                    })
                    .map_or((Config::starter(), None), |(c, p)| (c, Some(p)));

                if let Err(e) = self.save(&config) {
                    log::error!("could not write initial config: {e}");
                }
                match from {
                    Some(from) => LoadOutcome::Migrated { config, from },
                    None => LoadOutcome::Created(config),
                }
            }
            Err(e) => LoadOutcome::Invalid(StoreError::Io(self.path.clone(), e)),
        }
    }

    /// Atomically writes the config (write to a temp file, then rename).
    pub fn save(&self, config: &Config) -> Result<(), StoreError> {
        write_atomic(&self.path, &config.to_toml())
            .map_err(|e| StoreError::Io(self.path.clone(), e))
    }

    /// Moves an unreadable config aside so a fresh one can be written.
    /// Returns where the old file now lives.
    pub fn quarantine(&self) -> Result<PathBuf, StoreError> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        let dest = self.path.with_extension(format!("toml.broken-{stamp}"));
        fs::rename(&self.path, &dest).map_err(|e| StoreError::Io(self.path.clone(), e))?;
        Ok(dest)
    }
}

/// Reads a config to import: `.ini` files use the legacy format, anything
/// else is parsed as TOML.
pub fn import_file(path: &Path) -> Result<Config, StoreError> {
    let text = fs::read_to_string(path).map_err(|e| StoreError::Io(path.to_owned(), e))?;
    let is_ini = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ini"));
    if is_ini {
        from_legacy_ini(&text).map_err(|e| StoreError::Legacy(path.to_owned(), e))
    } else {
        Config::from_toml(&text).map_err(|e| StoreError::Toml(path.to_owned(), Box::new(e)))
    }
}

pub fn export_file(config: &Config, path: &Path) -> Result<(), StoreError> {
    write_atomic(path, &config.to_toml()).map_err(|e| StoreError::Io(path.to_owned(), e))
}

fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// The per-user folder for `config.toml`.
///
/// `%APPDATA%\LinuxLauncher` on Windows, `$XDG_CONFIG_HOME/linuxlauncher`
/// (or `~/.config/linuxlauncher`) elsewhere.
fn user_config_dir() -> Option<PathBuf> {
    let absolute = |v: std::ffi::OsString| Some(PathBuf::from(v)).filter(|p| p.is_absolute());
    if cfg!(windows) {
        std::env::var_os("APPDATA")
            .and_then(absolute)
            .map(|d| d.join(crate::APP_ID))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .and_then(absolute)
            .or_else(|| std::env::home_dir().map(|h| h.join(".config")))
            .map(|d| d.join(crate::APP_ID.to_lowercase()))
    }
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

/// Places where pre-2.0 versions kept their config.
fn legacy_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = exe_dir() {
        out.push(dir.join(LEGACY_FILE_NAME));
    }
    if let Some(home) = std::env::home_dir() {
        out.push(home.join(".linuxlauncher").join(LEGACY_FILE_NAME));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppEntry;

    #[cfg(windows)]
    #[test]
    fn user_config_dir_is_roaming_appdata() {
        let appdata = PathBuf::from(std::env::var_os("APPDATA").expect("APPDATA is set"));
        assert_eq!(user_config_dir(), Some(appdata.join(crate::APP_ID)));
    }

    #[test]
    fn creates_starter_config_on_first_run() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::at(dir.path().join("sub").join(CONFIG_FILE_NAME));
        let LoadOutcome::Created(config) = store.load_with_legacy(&[]) else {
            panic!("expected Created");
        };
        assert_eq!(config, Config::starter());
        assert!(store.path().exists());
        assert!(matches!(
            store.load_with_legacy(&[]),
            LoadOutcome::Loaded(_)
        ));
    }

    #[test]
    fn migrates_legacy_ini() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join(LEGACY_FILE_NAME);
        fs::write(&legacy, include_str!("../../tests/fixtures/legacy.ini")).unwrap();
        let store = ConfigStore::at(dir.path().join(CONFIG_FILE_NAME));

        let missing = dir.path().join("missing.ini");
        match store.load_with_legacy(&[missing, legacy.clone()]) {
            LoadOutcome::Migrated { config, from } => {
                assert_eq!(from, legacy);
                assert_eq!(config.apps.len(), 3);
            }
            other => panic!("expected Migrated, got {other:?}"),
        }
        assert!(legacy.exists(), "legacy file must be left in place");
    }

    #[test]
    fn invalid_config_is_reported_and_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        fs::write(&path, "apps = 'not a list'").unwrap();
        let store = ConfigStore::at(&path);
        assert!(matches!(
            store.load_with_legacy(&[]),
            LoadOutcome::Invalid(_)
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "apps = 'not a list'");

        let moved = store.quarantine().unwrap();
        assert!(moved.exists());
        assert!(!path.exists());
    }

    #[test]
    fn save_round_trips_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::at(dir.path().join(CONFIG_FILE_NAME));
        let mut config = Config::default();
        config.add_app(AppEntry::new("X", r"C:\x.exe"));
        store.save(&config).unwrap();
        store.save(&config).unwrap(); // overwrite existing
        let LoadOutcome::Loaded(loaded) = store.load_with_legacy(&[]) else {
            panic!("expected Loaded");
        };
        assert_eq!(loaded, config);
        let files: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn import_detects_format_by_extension() {
        let dir = tempfile::tempdir().unwrap();
        let ini = dir.path().join("old.ini");
        fs::write(&ini, "[a]\ntitle=A\ncommand=a.exe\n").unwrap();
        assert_eq!(import_file(&ini).unwrap().apps[0].name, "A");

        let toml = dir.path().join("export.toml");
        export_file(&Config::starter(), &toml).unwrap();
        assert_eq!(import_file(&toml).unwrap(), Config::starter());
    }
}
