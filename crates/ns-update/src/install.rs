//! Nagsworth installed by the setup, and the setup run for an update (Windows).

use std::io;
use std::path::PathBuf;

use crate::Setup;

/// How the setup installed this Nagsworth (Tauri's NSIS installer, `installMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Installed {
    /// Under `%LOCALAPPDATA%\Nagsworth`, the Settings > Apps entry in `HKCU`.
    CurrentUser,
    /// Under Program Files, the entry in `HKLM`.
    AllUsers,
}

/// The setup's Settings > Apps key (`src-tauri/tauri.conf.json` `productName`); its
/// `InstallLocation` is the folder, in quotes.
#[cfg(windows)]
const UNINST_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Nagsworth";

/// Whether this executable is one the setup installed: the Settings > Apps entry (one user's
/// first, then all users') whose `InstallLocation` is its folder. A copy run from anywhere
/// else (an unpacked build, `target\release`) is not: it is offered the release page instead.
#[cfg(windows)]
pub fn installed() -> Option<Installed> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_string_lossy().into_owned();
    [
        (HKEY_CURRENT_USER, Installed::CurrentUser),
        (HKEY_LOCAL_MACHINE, Installed::AllUsers),
    ]
    .into_iter()
    .find_map(|(root, mode)| {
        let location = read_reg_string(root, UNINST_KEY, "InstallLocation")?;
        same_folder(&location, &dir).then_some(mode)
    })
}

#[cfg(not(windows))]
pub fn installed() -> Option<Installed> {
    None
}

#[cfg(windows)]
fn read_reg_string(
    root: windows::Win32::System::Registry::HKEY,
    key: &str,
    value: &str,
) -> Option<String> {
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::PCWSTR;
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (key, value) = (wide(key), wide(value));
    let mut size = 0u32;
    // SAFETY: the first call queries the size in bytes, the second fills a buffer of that size.
    unsafe {
        if RegGetValueW(
            root,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        ) != ERROR_SUCCESS
        {
            return None;
        }
        let mut buf = vec![0u16; (size as usize).div_ceil(2)];
        if RegGetValueW(
            root,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        ) != ERROR_SUCCESS
        {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    }
}

/// Whether two folder paths name the same folder, as Windows compares them: without regard to
/// case, surrounding quotes or a trailing separator.
pub fn same_folder(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.trim()
            .trim_matches('"')
            .trim_end_matches(['\\', '/'])
            .replace('/', "\\")
            .to_lowercase()
    };
    let (a, b) = (norm(a), norm(b));
    !a.is_empty() && a == b
}

/// The setup's arguments for the update (Tauri's NSIS template and
/// `src-tauri/windows/hooks.nsh`): silent, update mode (no new shortcuts, the files replaced
/// in place), wait for this process to exit first, then start the new version hidden.
pub fn setup_args(pid: u32) -> [String; 6] {
    [
        "/S".to_owned(),
        "/UPDATE".to_owned(),
        "/R".to_owned(),
        format!("/WAITPID={pid}"),
        "/ARGS".to_owned(),
        "--hidden".to_owned(),
    ]
}

/// Where the setup is downloaded to (`%TEMP%\Nagsworth-update`); what an earlier update left
/// there goes first.
pub fn download_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("Nagsworth-update");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
    dir
}

/// Starts the setup for the update; it runs on while Nagsworth exits.
#[cfg(windows)]
pub fn run_setup(setup: &Setup) -> io::Result<()> {
    spawn_checked(setup, &setup_args(std::process::id())).map(drop)
}

#[cfg(not(windows))]
pub fn run_setup(_setup: &Setup) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "the setup runs on Windows only",
    ))
}

/// Opens `path` for reading so that nobody can write to it, rename it or delete it while the
/// handle is open (Windows still lets a process start from it: the loader only reads).
#[cfg(windows)]
fn open_locked(path: &std::path::Path) -> io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 1;
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(path)
}

/// Starts `setup` with `args` only if it still has the contents its download was checked
/// against: the file is checked again under [`open_locked`] and stays locked until the process
/// has started, so what runs is what was checked, however long the setup waited in `%TEMP%`
/// for the user's "install".
#[cfg(windows)]
fn spawn_checked(setup: &Setup, args: &[String]) -> io::Result<std::process::Child> {
    let file = open_locked(&setup.path)?;
    if !setup.matches(&file)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the file changed after it was checked",
        ));
    }
    let child = std::process::Command::new(&setup.path).args(args).spawn();
    drop(file);
    child
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_compare_as_windows_does() {
        assert!(same_folder(
            "\"C:\\Users\\x\\AppData\\Local\\Nagsworth\"",
            "c:/users/X/appdata/local/nagsworth\\"
        ));
        assert!(same_folder(
            "C:\\Program Files\\Nagsworth\\",
            "C:\\Program Files\\Nagsworth"
        ));
        assert!(!same_folder(
            "C:\\Program Files\\Nagsworth",
            "C:\\Program Files\\Nagsworth2"
        ));
        assert!(
            !same_folder("", ""),
            "no InstallLocation is no installation"
        );
        assert!(!same_folder("\"\"", ""));
    }

    #[test]
    fn the_setup_is_started_silently_in_update_mode() {
        assert_eq!(
            setup_args(4242),
            ["/S", "/UPDATE", "/R", "/WAITPID=4242", "/ARGS", "--hidden"]
        );
        // whatever this machine has: the lookup itself must not fail
        let _ = installed();
        assert!(download_dir().ends_with("Nagsworth-update"));
    }

    /// The setup runs only with the contents it was checked with, and nobody can change or
    /// replace the file between that check and the start (a copy of `cmd.exe` stands in for
    /// it: its exit code shows it ran with the arguments).
    #[cfg(windows)]
    #[test]
    fn the_setup_runs_only_as_checked() {
        use std::io::Write;
        let dir =
            std::env::temp_dir().join(format!("nagsworth-updater-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("setup.exe");
        let system = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        std::fs::copy(
            std::path::Path::new(&system)
                .join("System32")
                .join("cmd.exe"),
            &path,
        )
        .unwrap();
        let sha256 = crate::sha256_of(std::fs::File::open(&path).unwrap()).unwrap();
        let setup = Setup {
            path: path.clone(),
            sha256,
        };
        let args = ["/c".to_owned(), "exit".to_owned(), "7".to_owned()];
        let status = spawn_checked(&setup, &args).unwrap().wait().unwrap();
        assert_eq!(status.code(), Some(7));
        // while checked, the file cannot be written to, renamed or deleted
        let locked = open_locked(&path).unwrap();
        assert!(
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .is_err()
        );
        assert!(std::fs::rename(&path, dir.join("moved.exe")).is_err());
        assert!(std::fs::remove_file(&path).is_err());
        drop(locked);
        // changed after the download's check: refused, nothing started
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"x")
            .unwrap();
        let e = spawn_checked(&setup, &args).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidData, "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
