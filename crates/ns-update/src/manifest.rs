//! What the releases listing says.

use serde::Deserialize;

use crate::Version;

/// The newest release and the newest pre-release, from the releases listing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    /// The newest release.
    pub stable: Option<Version>,
    /// The newest pre-release: marked so on GitHub, or a version with a suffix.
    pub prerelease: Option<Version>,
}

/// One entry of GitHub's releases listing; the other fields are ignored.
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

impl Manifest {
    /// Reads GitHub's releases JSON (an array). Drafts and tags that are no `v<version>` are
    /// left out; anything that is no such array is an empty manifest, never an error, as a
    /// listing with no releases yet is.
    pub fn from_github_json(text: &str) -> Manifest {
        let mut m = Manifest::default();
        let Ok(releases) = serde_json::from_str::<Vec<Release>>(text) else {
            return m;
        };
        for r in releases {
            if r.draft {
                continue;
            }
            let Some(v) = Version::from_tag(&r.tag_name) else {
                continue;
            };
            let slot = if r.prerelease || v.is_prerelease() {
                &mut m.prerelease
            } else {
                &mut m.stable
            };
            if slot.as_ref().is_none_or(|s| v > *s) {
                *slot = Some(v);
            }
        }
        m
    }

    /// The newest version to offer to `current`, when one is newer: the release, and the
    /// pre-release with `beta` on or when `current` is a pre-release.
    pub fn newer(&self, current: &Version, beta: bool) -> Option<&Version> {
        let pre = self
            .prerelease
            .as_ref()
            .filter(|_| beta || current.is_prerelease());
        [self.stable.as_ref(), pre]
            .into_iter()
            .flatten()
            .filter(|v| *v > current)
            .max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn github_listing() {
        let json = r#"[
          {"tag_name": "v1.1.0-rc.2", "draft": false, "prerelease": true, "name": "rc"},
          {"tag_name": "v1.0.0", "draft": false, "prerelease": false},
          {"tag_name": "v0.9.0", "prerelease": false},
          {"tag_name": "v2.0.0", "draft": true, "prerelease": false},
          {"tag_name": "nightly", "prerelease": true},
          {"tag_name": "v1.0.1-beta", "prerelease": false}
        ]"#;
        let m = Manifest::from_github_json(json);
        assert_eq!(
            m,
            Manifest {
                stable: Some(v("1.0.0")),
                prerelease: Some(v("1.1.0-rc.2"))
            }
        );
        assert_eq!(Manifest::from_github_json("[]"), Manifest::default());
        assert_eq!(
            Manifest::from_github_json("<html>404</html>"),
            Manifest::default()
        );
        assert_eq!(
            Manifest::from_github_json(r#"{"message": "rate limited"}"#),
            Manifest::default()
        );
        // a release marked as such but with a suffix is a pre-release
        let m = Manifest::from_github_json(r#"[{"tag_name": "v1.0.1-beta", "prerelease": false}]"#);
        assert_eq!(m.stable, None);
        assert_eq!(m.prerelease, Some(v("1.0.1-beta")));
    }

    #[test]
    fn what_is_offered() {
        let both = Manifest {
            stable: Some(v("1.0.0")),
            prerelease: Some(v("1.1.0-rc.1")),
        };
        // a release user: the release only, unless pre-releases are on
        assert_eq!(both.newer(&v("0.9.0"), false), Some(&v("1.0.0")));
        assert_eq!(both.newer(&v("0.9.0"), true), Some(&v("1.1.0-rc.1")));
        assert_eq!(both.newer(&v("1.0.0"), false), None);
        assert_eq!(both.newer(&v("1.0.0"), true), Some(&v("1.1.0-rc.1")));
        // a release candidate's tester hears of the next candidate and of the release
        assert_eq!(both.newer(&v("1.0.0-rc.1"), false), Some(&v("1.1.0-rc.1")));
        let older_pre = Manifest {
            stable: Some(v("1.0.0")),
            prerelease: Some(v("1.0.0-rc.2")),
        };
        assert_eq!(older_pre.newer(&v("1.0.0-rc.1"), false), Some(&v("1.0.0")));
        assert_eq!(older_pre.newer(&v("1.0.0"), true), None);
        let pre_only = Manifest {
            stable: None,
            prerelease: Some(v("1.0.0-rc.2")),
        };
        assert_eq!(
            pre_only.newer(&v("1.0.0-rc.1"), false),
            Some(&v("1.0.0-rc.2"))
        );
        assert_eq!(pre_only.newer(&v("1.0.0-rc.2"), false), None);
        assert_eq!(Manifest::default().newer(&v("1.0.0"), true), None);
    }
}
