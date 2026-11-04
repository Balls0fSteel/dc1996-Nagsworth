//! A release's version, and where its files are.

use std::cmp::Ordering;
use std::fmt;

/// The repository, where the releases are.
pub const REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");
/// The releases page.
pub const RELEASES_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases");
/// The releases listing the check reads: GitHub's REST API, newest first, pre-releases
/// included, drafts left out for anyone not signed in.
pub const RELEASES_API_URL: &str =
    "https://api.github.com/repos/Balls0fSteel/dc1996-Nagsworth/releases?per_page=30";

/// A release's version: `MAJOR.MINOR.PATCH` with an optional `-pre.release` suffix, ordered as
/// Semantic Versioning orders them (`1.0.0-rc.2` < `1.0.0-rc.10` < `1.0.0`). Displayed as
/// written.
#[derive(Clone, Debug)]
pub struct Version {
    text: String,
    core: [u64; 3],
    pre: Vec<String>,
}

/// The longest version text taken: it goes into the dashboard, URLs and the setup's file name.
const VERSION_MAX: usize = 64;

impl Version {
    /// `1.0.0`, `1.0.0-rc.1`; `None` for anything else (a leading `v`, build metadata, empty
    /// identifiers, more than [`VERSION_MAX`] bytes).
    pub fn parse(s: &str) -> Option<Version> {
        if s.len() > VERSION_MAX {
            return None;
        }
        let (core_text, pre_text) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (s, None),
        };
        let mut core = [0; 3];
        let mut parts = core_text.split('.');
        for n in &mut core {
            let p = parts.next()?;
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            *n = p.parse().ok()?;
        }
        if parts.next().is_some() {
            return None;
        }
        let mut pre = Vec::new();
        if let Some(p) = pre_text {
            for id in p.split('.') {
                if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                    return None;
                }
                pre.push(id.to_owned());
            }
        }
        Some(Version {
            text: s.to_owned(),
            core,
            pre,
        })
    }

    /// A release's tag (`v1.2.3`) as a version.
    pub fn from_tag(tag: &str) -> Option<Version> {
        Version::parse(tag.strip_prefix('v')?)
    }

    /// The running Nagsworth's (the workspace version).
    pub fn current() -> Version {
        Version::parse(env!("CARGO_PKG_VERSION")).expect("the crate version is MAJOR.MINOR.PATCH")
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }

    /// The release's page on GitHub, where its files and notes are.
    pub fn release_url(&self) -> String {
        format!("{RELEASES_URL}/tag/v{}", self.text)
    }

    /// The release's Windows setup, as Tauri's bundler names it
    /// (`<productName>_<version>_<arch>-setup.exe`, `.github/workflows/release.yml`).
    pub fn setup_name(&self) -> String {
        format!("Nagsworth_{}_x64-setup.exe", self.text)
    }

    /// Where the release's file `name` is: the release's files on GitHub, or the folder
    /// `NAGSWORTH_UPDATE_FILES` names.
    pub fn file_url(&self, name: &str) -> String {
        match std::env::var("NAGSWORTH_UPDATE_FILES") {
            Ok(base) => format!("{}/{name}", base.trim_end_matches('/')),
            Err(_) => format!("{RELEASES_URL}/download/v{}/{name}", self.text),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Version) -> Ordering {
        self.core.cmp(&other.core).then_with(|| {
            match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                // a pre-release comes before its release
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => {
                    for (a, b) in self.pre.iter().zip(&other.pre) {
                        let o = match (a.parse::<u64>(), b.parse::<u64>()) {
                            (Ok(a), Ok(b)) => a.cmp(&b),
                            // numbers come before words
                            (Ok(_), Err(_)) => Ordering::Less,
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => a.cmp(b),
                        };
                        if o != Ordering::Equal {
                            return o;
                        }
                    }
                    self.pre.len().cmp(&other.pre.len())
                }
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Version) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Version {}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn versions_parse_and_print_as_written() {
        assert_eq!(v("1.0.0").to_string(), "1.0.0");
        assert_eq!(v("1.0.0-rc.1").to_string(), "1.0.0-rc.1");
        assert!(v("1.0.0-rc.1").is_prerelease() && !v("1.0.0").is_prerelease());
        assert_eq!(v("12.34.56-alpha-2.x").core, [12, 34, 56]);
        for bad in [
            "",
            "1",
            "1.0",
            "1.0.0.0",
            "v1.0.0",
            "1.0.0-",
            "1.0.0-rc..1",
            "1.0.0+build",
            "1.a.0",
            " 1.0.0",
            "1.0.0-rc_1",
        ] {
            assert!(Version::parse(bad).is_none(), "{bad}");
        }
        // a tampered listing's version of any length was taken, into the dashboard and file names
        let long = format!("1.0.0-{}", "a".repeat(VERSION_MAX - 6));
        assert_eq!(v(&long).to_string(), long);
        assert!(Version::parse(&format!("{long}a")).is_none());
        assert_eq!(Version::from_tag("v1.2.3"), Some(v("1.2.3")));
        assert_eq!(Version::from_tag("1.2.3"), None);
        assert_eq!(Version::current().to_string(), env!("CARGO_PKG_VERSION"));
        assert_eq!(
            v("1.0.0-rc.1").release_url(),
            "https://github.com/Balls0fSteel/dc1996-Nagsworth/releases/tag/v1.0.0-rc.1"
        );
    }

    #[test]
    fn semver_precedence() {
        // the example from the Semantic Versioning specification, item 11
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0-rc.2",
            "1.0.0-rc.10",
            "1.0.0",
            "1.0.1",
            "1.1.0-rc.1",
            "1.1.0",
            "2.0.0",
            "10.0.0",
        ];
        for pair in ordered.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
        }
        assert_eq!(v("1.0.0"), v("1.0.0"));
    }

    #[test]
    fn setup_names_and_urls() {
        let v = v("1.2.3");
        assert_eq!(v.setup_name(), "Nagsworth_1.2.3_x64-setup.exe");
        assert_eq!(
            v.file_url("SHA256SUMS"),
            "https://github.com/Balls0fSteel/dc1996-Nagsworth/releases/download/v1.2.3/SHA256SUMS"
        );
    }
}
