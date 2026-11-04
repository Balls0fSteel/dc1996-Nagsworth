//! Steam library files: `libraryfolders.vdf` lists library folders, and each
//! library's `steamapps/appmanifest_<id>.acf` holds the game's name. Both are
//! Valve KeyValues text; only quoted key/value pairs are needed here.

use std::path::{Path, PathBuf};

/// Quoted strings of a KeyValues file, in order (braces dropped).
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut s = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(n) = chars.next() {
                        s.push(n);
                    }
                }
                '"' => break,
                c => s.push(c),
            }
        }
        out.push(s);
    }
    out
}

/// Values of every `"key" "value"` pair with this key.
fn values<'a>(tokens: &'a [String], key: &str) -> impl Iterator<Item = &'a str> {
    tokens
        .windows(2)
        .filter(move |w| w[0].eq_ignore_ascii_case(key))
        .map(|w| w[1].as_str())
}

/// Library folders, the Steam folder itself first.
pub fn library_folders(steam_path: &Path) -> Vec<PathBuf> {
    let mut out = vec![steam_path.to_path_buf()];
    let file = steam_path.join("steamapps").join("libraryfolders.vdf");
    if let Ok(text) = std::fs::read_to_string(file) {
        for p in values(&tokens(&text), "path") {
            let p = PathBuf::from(p);
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// The name of a Steam app from its manifest in any library.
pub fn app_name(steam_path: &Path, app_id: u32) -> Option<String> {
    library_folders(steam_path).into_iter().find_map(|lib| {
        let manifest = lib
            .join("steamapps")
            .join(format!("appmanifest_{app_id}.acf"));
        let text = std::fs::read_to_string(manifest).ok()?;
        values(&tokens(&text), "name").next().map(str::to_string)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_names_across_libraries() {
        let root = std::env::temp_dir().join(format!("an-steam-test-{}", std::process::id()));
        let steam = root.join("Steam");
        let lib2 = root.join("Lib2");
        std::fs::create_dir_all(steam.join("steamapps")).unwrap();
        std::fs::create_dir_all(lib2.join("steamapps")).unwrap();
        let lib2_escaped = lib2.display().to_string().replace('\\', "\\\\");
        std::fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{lib2_escaped}\"\n\t\t\"apps\" {{ \"1145360\" \"123\" }}\n\t}}\n}}\n"
            ),
        )
        .unwrap();
        std::fs::write(
            lib2.join("steamapps/appmanifest_1145360.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"1145360\"\n\t\"name\"\t\t\"Hades \\\"II\\\"\"\n}\n",
        )
        .unwrap();
        assert_eq!(library_folders(&steam).len(), 2);
        assert_eq!(app_name(&steam, 1145360).as_deref(), Some("Hades \"II\""));
        assert_eq!(app_name(&steam, 1), None);
        let _ = std::fs::remove_dir_all(root);
    }
}
