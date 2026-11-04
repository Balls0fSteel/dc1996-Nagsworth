//! The TOML config file. Rules live in the same file but are parsed by
//! `ns-rules`; this module ignores the `[[rules]]` tables.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::day::TimeOfDay;

/// The shipped config, also used when the user's file is missing.
pub const DEFAULT_CONFIG_TOML: &str = include_str!("../../../config/default.toml");

pub const APP_DIR_NAME: &str = "nagsworth";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub tracking: Tracking,
    pub web: Web,
    pub api: Api,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub day_start: TimeOfDay,
    pub reading_mode: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            day_start: TimeOfDay::new(4, 0).expect("valid time"),
            reading_mode: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreTitles {
    #[default]
    Full,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tracking {
    pub poll_ms: u64,
    pub idle_after_s: u64,
    pub min_segment_ms: u64,
    pub store_titles: StoreTitles,
    pub retention_days: u32,
    /// Seconds that one key press, mouse move or controller input counts as
    /// hands-on time. 0 disables hands-on accounting.
    pub input_window_s: u64,
    /// Rules for the Game category count hands-on time by default
    /// (`idle_policy = "input"` instead of `"ignore"`).
    pub hands_on_games: bool,
    /// Poll XInput controllers (Windows) so gamepad use counts as input.
    pub gamepad: bool,
}

/// Longest `input_window_s` the config accepts.
pub const MAX_INPUT_WINDOW_S: u64 = 600;

impl Default for Tracking {
    fn default() -> Self {
        Self {
            poll_ms: 1000,
            idle_after_s: 120,
            min_segment_ms: 2000,
            store_titles: StoreTitles::Full,
            retention_days: 365,
            input_window_s: 5,
            hands_on_games: true,
            gamepad: true,
        }
    }
}

impl Tracking {
    /// `input_window_s` in milliseconds, clamped to [`MAX_INPUT_WINDOW_S`].
    pub fn input_window_ms(&self) -> u64 {
        self.input_window_s.min(MAX_INPUT_WINDOW_S) * 1000
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreUrl {
    #[default]
    Host,
    Full,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Web {
    pub store_url: StoreUrl,
    pub include_incognito: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Api {
    pub port: u16,
    pub bind: String,
}

impl Default for Api {
    fn default() -> Self {
        Self {
            port: 47821,
            bind: "127.0.0.1".into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("invalid config: {0}")]
    Parse(#[from] toml::de::Error),
}

impl Config {
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        Ok(toml::from_str(text)?)
    }
}

/// A loaded config plus the raw text, which `ns-rules` parses for rules.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub config: Config,
    pub text: String,
    pub path: Option<PathBuf>,
}

/// Loads `path`, falling back to the bundled default when it does not exist.
pub fn load(path: &Path) -> Result<Loaded, ConfigError> {
    let (text, found) = match std::fs::read_to_string(path) {
        Ok(text) => (text, Some(path.to_path_buf())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => (DEFAULT_CONFIG_TOML.to_string(), None),
        Err(source) => {
            return Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    Ok(Loaded {
        config: Config::from_toml(&text)?,
        text,
        path: found,
    })
}

/// Writes the bundled default config to `path` if no file exists there.
pub fn ensure_default_file(path: &Path) -> io::Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, DEFAULT_CONFIG_TOML)?;
    Ok(true)
}

/// Per-user data directory: `NAGSWORTH_DATA_DIR` if set, else
/// `%LOCALAPPDATA%\nagsworth` on Windows and
/// `$XDG_DATA_HOME/nagsworth` (or `~/.local/share/...`) elsewhere.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("NAGSWORTH_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    base.unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR_NAME)
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_default_matches_default_impl() {
        let parsed = Config::from_toml(DEFAULT_CONFIG_TOML).unwrap();
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn config_round_trips() {
        let mut c = Config::default();
        c.tracking.store_titles = StoreTitles::None;
        c.general.day_start = "05:30".parse().unwrap();
        c.api.port = 50000;
        let text = toml::to_string(&c).unwrap();
        assert_eq!(Config::from_toml(&text).unwrap(), c);
    }

    #[test]
    fn missing_keys_take_defaults() {
        let c = Config::from_toml("[tracking]\npoll_ms = 500\n").unwrap();
        assert_eq!(c.tracking.poll_ms, 500);
        assert_eq!(c.tracking.idle_after_s, 120);
        assert_eq!(c.tracking.input_window_s, 5);
        assert!(c.tracking.hands_on_games);
        assert!(c.tracking.gamepad);
        assert_eq!(c.api, Api::default());
    }

    #[test]
    fn input_window_is_clamped() {
        let c = Config::from_toml("[tracking]
input_window_s = 100000
").unwrap();
        assert_eq!(c.tracking.input_window_ms(), MAX_INPUT_WINDOW_S * 1000);
        let c = Config::from_toml("[tracking]
input_window_s = 0
").unwrap();
        assert_eq!(c.tracking.input_window_ms(), 0);
    }

    #[test]
    fn load_falls_back_to_default() {
        let loaded = load(Path::new("definitely/not/here.toml")).unwrap();
        assert!(loaded.path.is_none());
        assert_eq!(loaded.text, DEFAULT_CONFIG_TOML);
    }
}
