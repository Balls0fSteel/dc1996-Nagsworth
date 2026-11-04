//! HTTP: the releases listing, and a release's setup checked against its `SHA256SUMS`.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::{Manifest, RELEASES_API_URL, Version};

/// A SHA-256 as bytes.
pub type Sha256Hash = [u8; 32];

/// The listing at most: thirty releases are a few hundred KB.
const LISTING_MAX: u64 = 1 << 20;
/// The setup's size at most: the release's is a few MB.
const SETUP_MAX: u64 = 64 << 20;

/// Whether a hand test redirected the URLs (then plain `http://` is allowed, for a local
/// server).
fn test_override() -> bool {
    std::env::var_os("NAGSWORTH_UPDATE_URL").is_some()
        || std::env::var_os("NAGSWORTH_UPDATE_FILES").is_some()
}

fn agent(body_timeout: Duration) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .https_only(!test_override())
        .max_redirects(5)
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(body_timeout))
        .user_agent(format!("nagsworth/{}", env!("CARGO_PKG_VERSION")))
        .build();
    ureq::Agent::new_with_config(config)
}

fn get_text(agent: &ureq::Agent, url: &str, max: u64) -> Result<String, String> {
    let mut resp = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .map_err(|e| e.to_string())?;
    resp.body_mut()
        .with_config()
        .limit(max)
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// Fetches and reads the releases listing (blocking: call it from a worker thread).
pub fn fetch() -> Result<Manifest, String> {
    let url = std::env::var("NAGSWORTH_UPDATE_URL").unwrap_or_else(|_| RELEASES_API_URL.to_owned());
    let text = get_text(&agent(Duration::from_secs(30)), &url, LISTING_MAX)?;
    Ok(Manifest::from_github_json(&text))
}

/// The SHA-256 a `SHA256SUMS` file (`sha256sum`'s `<hex>  <name>` lines, a `*` before a binary
/// name) gives for `name`, if it lists it.
pub fn expected_sha256(sums: &str, name: &str) -> Option<Sha256Hash> {
    sums.lines().find_map(|line| {
        let (hex, file) = line.trim().split_once(' ')?;
        let file = file.trim_start().trim_start_matches('*');
        (file == name).then(|| parse_hex(hex)).flatten()
    })
}

fn parse_hex(hex: &str) -> Option<Sha256Hash> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// A downloaded setup and the SHA-256 it was checked against, to check it again just before it
/// runs ([`Setup::matches`]): the file sits in a folder for as long as the user takes to say
/// "install".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setup {
    pub path: PathBuf,
    pub sha256: Sha256Hash,
}

impl Setup {
    /// Whether `file` (the setup, opened) still has the checked contents.
    pub fn matches(&self, file: impl Read) -> io::Result<bool> {
        Ok(sha256_of(file)? == self.sha256)
    }
}

/// The SHA-256 of everything `r` reads.
pub fn sha256_of(mut r: impl Read) -> io::Result<Sha256Hash> {
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        match r.read(&mut buf) {
            Ok(0) => return Ok(hash.finalize().into()),
            Ok(n) => hash.update(&buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}

/// Downloads `v`'s Windows setup into `dir` (made if missing) and checks it against the
/// release's `SHA256SUMS`, both over HTTPS (a redirect to plain http is refused); the setup's
/// path and checksum on success. `progress` hears each piece's running total and the size when
/// known, and ends the download by answering `false`. The file is written as a new
/// `<name>.part` (whatever had that name goes first, so a link planted there is not written
/// through) and renamed once it is checked; a failed or stopped download leaves nothing
/// behind. Blocking: a worker thread's.
pub fn download_setup(
    v: &Version,
    dir: &Path,
    progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<Setup, String> {
    let name = v.setup_name();
    let agent = agent(Duration::from_secs(15 * 60));
    let sums = get_text(&agent, &v.file_url("SHA256SUMS"), 64 * 1024)
        .map_err(|e| format!("SHA256SUMS: {e}"))?;
    let expected =
        expected_sha256(&sums, &name).ok_or_else(|| format!("SHA256SUMS does not list {name}"))?;

    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let part = dir.join(format!("{name}.part"));
    let path = dir.join(&name);
    let result = (|| {
        let _ = std::fs::remove_file(&part);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part)
            .map_err(|e| format!("{}: {e}", part.display()))?;
        let mut resp = agent
            .get(&v.file_url(&name))
            .call()
            .map_err(|e| e.to_string())?;
        let total = resp.body().content_length();
        let mut reader = resp.body_mut().with_config().limit(SETUP_MAX).reader();
        let mut hash = Sha256::new();
        let mut got = 0u64;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.to_string()),
            };
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            hash.update(&buf[..n]);
            got += n as u64;
            if !progress(got, total) {
                return Err("cancelled".to_owned());
            }
        }
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        if Sha256Hash::from(hash.finalize()) != expected {
            return Err("the downloaded file's checksum does not match SHA256SUMS".to_owned());
        }
        let _ = std::fs::remove_file(&path);
        std::fs::rename(&part, &path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Setup {
            path,
            sha256: expected,
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result.map_err(|e| format!("{name}: {e}"))
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn hex(h: &Sha256Hash) -> String {
        h.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn the_checksum_list() {
        let sums = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  Nagsworth_1.2.3_x64-setup.exe.zip\n\
                    fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210 *Nagsworth_1.2.3_x64-setup.exe\r\n\
                    not-a-hash  other\n";
        let h = expected_sha256(sums, "Nagsworth_1.2.3_x64-setup.exe").unwrap();
        assert_eq!(
            hex(&h),
            "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
        );
        assert!(expected_sha256(sums, "Nagsworth_1.2.3_x64-setup.exe.zip").is_some());
        assert!(expected_sha256(sums, "other").is_none());
        assert!(expected_sha256(sums, "Nagsworth_1.2.4_x64-setup.exe").is_none());
        assert_eq!(
            hex(&sha256_of(&b""[..]).unwrap()),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// A tiny HTTP/1.1 server on a thread: answers each request with the next body.
    fn serve(bodies: Vec<Vec<u8>>) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        std::thread::spawn(move || {
            for body in bodies {
                let (mut s, _) = l.accept().unwrap();
                let mut head = Vec::new();
                let mut b = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") && s.read(&mut b).unwrap_or(0) == 1 {
                    head.push(b[0]);
                }
                let _ = s.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                );
                let _ = s.write_all(&body);
            }
        });
        base
    }

    /// The listing and the setup download from the places the two env vars name; the setup is
    /// checked against SHA256SUMS, and a wrong checksum or a cancel leaves no file. One test
    /// for both, as the env vars are process-wide.
    #[test]
    fn the_listing_and_the_setup_download_and_are_checked() {
        let setup = vec![7u8; 100_000];
        let good: Sha256Hash = Sha256::digest(&setup).into();
        let name = v("9.9.9").setup_name();
        let sums_ok = format!("{}  {name}\n", hex(&good)).into_bytes();
        let sums_bad = format!("{}  {name}\n", "0".repeat(64)).into_bytes();
        let listing = br#"[{"tag_name": "v9.9.9", "prerelease": false}]"#.to_vec();
        // the listing; then SHA256SUMS and the setup three times: good, bad checksum, cancelled
        let base = serve(vec![
            listing,
            sums_ok.clone(),
            setup.clone(),
            sums_bad,
            setup.clone(),
            sums_ok,
            setup.clone(),
        ]);
        unsafe {
            std::env::set_var("NAGSWORTH_UPDATE_URL", format!("{base}/releases.json"));
            std::env::set_var("NAGSWORTH_UPDATE_FILES", &base);
        }
        let m = fetch().unwrap();
        assert_eq!(m.stable, Some(v("9.9.9")));

        let dir =
            std::env::temp_dir().join(format!("nagsworth-update-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // a link planted as the part file is not written through: its target keeps its bytes
        std::fs::create_dir_all(&dir).unwrap();
        let victim = dir.join("victim.txt");
        std::fs::write(&victim, b"keep").unwrap();
        std::fs::hard_link(&victim, dir.join(format!("{name}.part"))).unwrap();
        let mut seen = Vec::new();
        let got = download_setup(&v("9.9.9"), &dir, &mut |got, total| {
            seen.push((got, total));
            true
        })
        .unwrap();
        assert_eq!(
            got,
            Setup {
                path: dir.join(&name),
                sha256: good
            }
        );
        assert_eq!(std::fs::read(&got.path).unwrap(), setup);
        assert_eq!(std::fs::read(&victim).unwrap(), b"keep");
        assert_eq!(seen.last(), Some(&(100_000, Some(100_000))));
        assert!(seen.iter().all(|(_, t)| *t == Some(100_000)));
        // checked again before it runs: a file changed since no longer matches
        assert!(
            got.matches(std::fs::File::open(&got.path).unwrap())
                .unwrap()
        );
        std::fs::OpenOptions::new()
            .append(true)
            .open(&got.path)
            .unwrap()
            .write_all(b"x")
            .unwrap();
        assert!(
            !got.matches(std::fs::File::open(&got.path).unwrap())
                .unwrap()
        );
        let e = download_setup(&v("9.9.9"), &dir, &mut |_, _| true).unwrap_err();
        assert!(e.contains("checksum"), "{e}");
        assert!(!dir.join(format!("{name}.part")).exists());
        let e = download_setup(&v("9.9.9"), &dir, &mut |_, _| false).unwrap_err();
        assert!(e.contains("cancelled"), "{e}");
        assert!(!dir.join(format!("{name}.part")).exists());
        unsafe {
            std::env::remove_var("NAGSWORTH_UPDATE_URL");
            std::env::remove_var("NAGSWORTH_UPDATE_FILES");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
