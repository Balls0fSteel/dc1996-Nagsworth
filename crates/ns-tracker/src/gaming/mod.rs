//! Game detection without touching game processes: Steam's own registry
//! state, launcher install paths and a list of well-known game executables.

pub mod steam;

/// Lower-cased exe name without a `.exe` suffix, so Windows and Linux names
/// compare equal (`Firefox.exe` and `firefox`).
pub fn exe_stem(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    lower
        .strip_suffix(".exe")
        .map(str::to_string)
        .unwrap_or(lower)
}

use ns_core::ProcessInfo;
use ns_platform::SteamInfo;

/// Why a process was considered a game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameEvidence {
    Steam,
    LauncherPath,
    KnownExe,
}

/// A process under `steamapps\common` while Steam reports a running app.
#[derive(Debug, Default)]
pub struct SteamDetector;

impl SteamDetector {
    pub fn detect(&self, process: &ProcessInfo, steam: Option<&SteamInfo>) -> bool {
        let Some(steam) = steam else { return false };
        steam.running_app_id != 0 && path_contains(process, "\\steamapps\\common\\")
    }
}

/// Install folders of the common launchers.
#[derive(Debug)]
pub struct LauncherPathDetector {
    fragments: Vec<&'static str>,
}

impl Default for LauncherPathDetector {
    fn default() -> Self {
        Self {
            fragments: vec![
                "\\steamapps\\common\\",
                "\\epic games\\",
                "\\gog galaxy\\games\\",
                "\\gog games\\",
                "\\riot games\\",
                "\\ubisoft game launcher\\games\\",
                "\\ea games\\",
                "\\origin games\\",
                "\\battle.net\\",
                "\\xboxgames\\",
                "\\amazon games\\library\\",
                "\\itch\\apps\\",
            ],
        }
    }
}

impl LauncherPathDetector {
    pub fn detect(&self, process: &ProcessInfo) -> bool {
        let Some(path) = &process.exe_path else {
            return false;
        };
        let path = path.replace('/', "\\").to_ascii_lowercase();
        // The launchers themselves live next to their games; they are not games.
        if KnownExeList::LAUNCHERS
            .iter()
            .any(|l| exe_stem(l) == exe_stem(&process.exe_name))
        {
            return false;
        }
        self.fragments.iter().any(|f| path.contains(f))
    }
}

/// Executables that are games wherever they are installed.
#[derive(Debug)]
pub struct KnownExeList {
    exes: Vec<String>,
}

impl KnownExeList {
    pub const LAUNCHERS: &'static [&'static str] = &[
        "steam.exe",
        "steamwebhelper.exe",
        "epicgameslauncher.exe",
        "galaxyclient.exe",
        "riotclientservices.exe",
        "upc.exe",
        "eadesktop.exe",
        "battle.net.exe",
    ];

    const BUILTIN: &'static [&'static str] = &[
        "minecraft.exe",
        "javaw.exe@minecraft",
        "leagueclient.exe",
        "league of legends.exe",
        "valorant.exe",
        "valorant-win64-shipping.exe",
        "cs2.exe",
        "dota2.exe",
        "fortnitelauncher.exe",
        "fortniteclient-win64-shipping.exe",
        "overwatch.exe",
        "wow.exe",
        "hearthstone.exe",
        "rocketleague.exe",
        "gta5.exe",
        "eldenring.exe",
        "factorio.exe",
        "terraria.exe",
        "robloxplayerbeta.exe",
    ];

    pub fn builtin() -> Self {
        Self::new(Self::BUILTIN.iter().map(|s| s.to_string()))
    }

    /// Entries are exe names; `exe@fragment` also requires the exe path to
    /// contain `fragment` (for shared runtimes like javaw.exe).
    pub fn new(exes: impl IntoIterator<Item = String>) -> Self {
        Self {
            exes: exes.into_iter().map(|e| e.to_ascii_lowercase()).collect(),
        }
    }

    /// The built-in list plus `games = [...]` from a user file
    /// (`known_games.toml` in the data directory). A missing file is fine.
    pub fn builtin_and_file(path: &std::path::Path) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct File {
            #[serde(default)]
            games: Vec<String>,
        }
        let mut list = Self::builtin();
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let file: File =
                    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
                list.exes
                    .extend(file.games.into_iter().map(|g| g.to_ascii_lowercase()));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
        Ok(list)
    }

    pub fn detect(&self, process: &ProcessInfo) -> bool {
        let exe = exe_stem(&process.exe_name);
        let path = process
            .exe_path
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        self.exes.iter().any(|entry| match entry.split_once('@') {
            Some((name, fragment)) => exe_stem(name) == exe && path.contains(fragment),
            None => exe_stem(entry) == exe,
        })
    }
}

impl Default for KnownExeList {
    fn default() -> Self {
        Self::builtin()
    }
}

fn path_contains(process: &ProcessInfo, fragment: &str) -> bool {
    process
        .exe_path
        .as_deref()
        .map(|p| p.replace('/', "\\").to_ascii_lowercase().contains(fragment))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(exe: &str, path: &str) -> ProcessInfo {
        ProcessInfo {
            pid: 1,
            exe_name: exe.into(),
            exe_path: Some(path.into()),
            is_elevated: false,
            package_id: None,
        }
    }

    #[test]
    fn detects_games() {
        let steam_game = p(
            "Hades.exe",
            "D:\\Steam\\steamapps\\common\\Hades\\x64\\Hades.exe",
        );
        let running = SteamInfo {
            running_app_id: 1145360,
            steam_path: None,
        };
        assert!(SteamDetector.detect(&steam_game, Some(&running)));
        assert!(!SteamDetector.detect(&steam_game, Some(&SteamInfo::default())));
        assert!(LauncherPathDetector::default().detect(&steam_game));
        let epic_launcher = p(
            "EpicGamesLauncher.exe",
            "C:\\Program Files\\Epic Games\\Launcher\\EpicGamesLauncher.exe",
        );
        assert!(!LauncherPathDetector::default().detect(&epic_launcher));
        assert!(!LauncherPathDetector::default().detect(&p("code.exe", "C:\\VS Code\\code.exe")));
        let known = KnownExeList::builtin();
        assert!(known.detect(&p("CS2.exe", "X:\\cs2.exe")));
        assert!(known.detect(&p(
            "javaw.exe",
            "C:\\Users\\x\\.minecraft\\runtime\\javaw.exe"
        )));
        assert!(!known.detect(&p("javaw.exe", "C:\\jdk\\bin\\javaw.exe")));

        let file = std::env::temp_dir().join(format!("an-known-{}.toml", std::process::id()));
        std::fs::write(&file, "games = [\"MyIndie.exe\"]\n").unwrap();
        let user = KnownExeList::builtin_and_file(&file).unwrap();
        assert!(user.detect(&p("myindie.exe", "C:\\x\\MyIndie.exe")));
        assert!(user.detect(&p("cs2.exe", "X:\\cs2.exe")));
        std::fs::write(&file, "games = 3").unwrap();
        assert!(KnownExeList::builtin_and_file(&file).is_err());
        let _ = std::fs::remove_file(&file);
        assert!(KnownExeList::builtin_and_file(std::path::Path::new("nope/x.toml")).is_ok());
    }
}
