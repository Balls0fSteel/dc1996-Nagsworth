//! Default category for apps seen for the first time. A category the user
//! picked is stored in the database and always wins over this.

use ns_core::{CategoryKind, ProcessInfo};
use ns_platform::SteamInfo;

use crate::gaming::{KnownExeList, LauncherPathDetector, SteamDetector, exe_stem};

pub const BROWSERS: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "opera_gx.exe",
    "vivaldi.exe",
    "librewolf.exe",
    "waterfox.exe",
    "floorp.exe",
    "zen.exe",
    "arc.exe",
    "chromium.exe",
    // Linux binary names.
    "firefox-bin",
    "google-chrome",
    "chromium-browser",
    "brave-browser",
    "microsoft-edge",
    "vivaldi-bin",
];

const WORK: &[&str] = &[
    "code.exe",
    "cursor.exe",
    "devenv.exe",
    "idea64.exe",
    "pycharm64.exe",
    "rider64.exe",
    "clion64.exe",
    "webstorm64.exe",
    "goland64.exe",
    "studio64.exe",
    "windowsterminal.exe",
    "wezterm-gui.exe",
    "alacritty.exe",
    "powershell.exe",
    "pwsh.exe",
    "cmd.exe",
    "winword.exe",
    "excel.exe",
    "powerpnt.exe",
    "outlook.exe",
    "onenote.exe",
    "ms-teams.exe",
    "teams.exe",
    "slack.exe",
    "zoom.exe",
    "notepad.exe",
    "notepad++.exe",
    "sublime_text.exe",
    "obsidian.exe",
    "blender.exe",
    "claude.exe",
];

/// `is_browser` for an exe name.
pub fn is_browser(exe_name: &str) -> bool {
    let exe = exe_stem(exe_name);
    BROWSERS.iter().any(|b| exe_stem(b) == exe)
}

/// Short browser family name for an exe, matching what the extension reports.
pub fn browser_family(exe_name: &str) -> Option<&'static str> {
    let exe = exe_stem(exe_name);
    Some(match exe.as_str() {
        "chrome" | "chromium" | "google-chrome" | "chromium-browser" => "chrome",
        "msedge" | "microsoft-edge" => "edge",
        "firefox" | "firefox-bin" | "librewolf" | "waterfox" | "floorp" | "zen" => "firefox",
        "brave" | "brave-browser" => "brave",
        "opera" | "opera_gx" => "opera",
        "vivaldi" | "vivaldi-bin" => "vivaldi",
        "arc" => "arc",
        _ => return None,
    })
}

/// Order: known games > Steam > launcher paths > browsers/work lists > Other.
#[derive(Debug, Default)]
pub struct Categorizer {
    known: KnownExeList,
    steam: SteamDetector,
    launchers: LauncherPathDetector,
}

impl Categorizer {
    pub fn new(known: KnownExeList) -> Self {
        Self {
            known,
            ..Default::default()
        }
    }

    /// Returns the default category and whether the app is a game.
    pub fn categorize(
        &self,
        process: &ProcessInfo,
        steam: Option<&SteamInfo>,
    ) -> (CategoryKind, bool) {
        if self.known.detect(process)
            || self.steam.detect(process, steam)
            || self.launchers.detect(process)
        {
            return (CategoryKind::Game, true);
        }
        let exe = process.exe_name.as_str();
        if is_browser(exe) {
            (CategoryKind::Web, false)
        } else if WORK.iter().any(|w| exe_stem(w) == exe_stem(exe)) {
            (CategoryKind::Work, false)
        } else {
            (CategoryKind::Other, false)
        }
    }
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
    fn categorizes_by_priority() {
        let c = Categorizer::default();
        assert_eq!(
            c.categorize(&p("Code.exe", "C:\\code.exe"), None),
            (CategoryKind::Work, false)
        );
        assert_eq!(
            c.categorize(&p("firefox.exe", "C:\\ff\\firefox.exe"), None),
            (CategoryKind::Web, false)
        );
        assert_eq!(
            c.categorize(&p("x.exe", "E:\\Epic Games\\X\\x.exe"), None),
            (CategoryKind::Game, true)
        );
        assert_eq!(
            c.categorize(&p("mystery.exe", "C:\\m.exe"), None),
            (CategoryKind::Other, false)
        );
        assert_eq!(browser_family("MSEDGE.exe"), Some("edge"));
        assert_eq!(browser_family("code.exe"), None);
        // Linux names match without the .exe suffix.
        assert_eq!(browser_family("firefox"), Some("firefox"));
        assert_eq!(
            c.categorize(&p("code", "/usr/share/code/code"), None),
            (CategoryKind::Work, false)
        );
        assert_eq!(
            c.categorize(
                &p("game", "/home/x/.steam/steam/steamapps/common/G/game"),
                None
            ),
            (CategoryKind::Game, true)
        );
    }
}
