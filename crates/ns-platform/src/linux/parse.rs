//! Pure parsers for Linux files, compiled on every OS so the tests run
//! everywhere.

/// Effective uid from the text of `/proc/<pid>/status`.
pub fn effective_uid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))
        .and_then(|rest| rest.split_whitespace().nth(1))
        .and_then(|uid| uid.parse().ok())
}

/// `RunningAppID` from Steam's `registry.vdf` (0 when nothing runs).
pub fn steam_running_app_id(registry_vdf: &str) -> Option<u32> {
    let mut quoted = registry_vdf.split('"').skip(1).step_by(2);
    while let Some(tok) = quoted.next() {
        if tok.eq_ignore_ascii_case("RunningAppID") {
            return quoted.next().and_then(|v| v.parse().ok());
        }
    }
    None
}

/// An XDG autostart entry starting `exe` hidden.
pub fn autostart_desktop_entry(exe: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=nagsworth\nExec=\"{}\" --autostart\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n",
        exe.replace('"', "\\\"")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_status() {
        let status = "Name:\tbash\nUid:\t1000\t0\t0\t0\nGid:\t1000\t1000\t1000\t1000\n";
        assert_eq!(effective_uid(status), Some(0));
        assert_eq!(effective_uid("Name: x\n"), None);
    }

    #[test]
    fn parses_steam_registry() {
        let vdf = r#""Registry"
{
	"HKCU"
	{
		"Software"
		{
			"Valve"
			{
				"Steam"
				{
					"RunningAppID"		"1145360"
					"language"		"english"
				}
			}
		}
	}
}"#;
        assert_eq!(steam_running_app_id(vdf), Some(1145360));
        assert_eq!(steam_running_app_id("\"x\" \"y\""), None);
    }

    #[test]
    fn writes_desktop_entry() {
        let e = autostart_desktop_entry("/opt/nagsworth/nagsworth");
        assert!(e.contains("Exec=\"/opt/nagsworth/nagsworth\" --autostart"));
    }
}
