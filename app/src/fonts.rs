//! Fonts installed on this Windows machine (for all users and for this user), read from the
//! registry where Windows lists them. They are loaded from the machine's own font files when
//! picked, never copied into Etapix.

use std::path::PathBuf;

/// A family and the files of its regular and bold faces.
#[derive(Clone)]
pub struct Family {
    pub name: String,
    pub regular: PathBuf,
    pub bold: Option<PathBuf>,
}

const STYLES: [&str; 14] = [
    "Bold", "Italic", "Oblique", "Light", "Thin", "Medium", "Semibold", "SemiBold", "Black", "Heavy", "Condensed", "ExtraLight",
    "SemiLight", "Semilight",
];

/// Registry entries as (display name, file path).
fn entries() -> Vec<(String, PathBuf)> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let mut out = Vec::new();
    for key in [r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts", r"HKCU\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts"] {
        let Ok(o) = std::process::Command::new("reg").args(["query", key]).creation_flags(CREATE_NO_WINDOW).output() else {
            continue;
        };
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let Some((name, file)) = line.trim().split_once("    REG_SZ    ") else { continue };
            let file = file.trim();
            let lower = file.to_lowercase();
            if !(lower.ends_with(".ttf") || lower.ends_with(".otf") || lower.ends_with(".ttc")) {
                continue;
            }
            let path = if file.contains('\\') { PathBuf::from(file) } else { PathBuf::from(&windir).join("Fonts").join(file) };
            let name = name.trim().trim_end_matches(" (TrueType)").trim_end_matches(" (OpenType)").to_string();
            out.push((name, path));
        }
    }
    out
}

/// Every family with a regular face, sorted by name; its bold face when there is one.
pub fn installed() -> Vec<Family> {
    let entries = entries();
    let mut families: Vec<Family> = entries
        .iter()
        .filter(|(name, _)| !name.split(' ').any(|w| STYLES.contains(&w)))
        .map(|(name, path)| {
            // "Cambria & Cambria Math" is one file with two families; list the first.
            let family = name.split(" & ").next().unwrap_or(name).to_string();
            let bold = entries.iter().find(|(n, _)| *n == format!("{family} Bold")).map(|(_, p)| p.clone());
            Family { name: family, regular: path.clone(), bold }
        })
        .filter(|f| f.regular.exists())
        .collect();
    families.sort_by_key(|f| f.name.to_lowercase());
    families.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    families
}

/// The settings file: `%APPDATA%\Etapix\settings.txt`, `key=value` lines.
fn settings_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Etapix").join("settings.txt"))
}

/// The saved font choice: family name (empty for the brand font) and text size.
pub fn load_choice() -> (String, f32) {
    let text = settings_path().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    let get = |k: &str| text.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(str::trim).unwrap_or("").to_string();
    (get("font"), get("font_size").parse().unwrap_or(12.0))
}

pub fn save_choice(family: &str, size: f32) {
    if let Some(p) = settings_path() {
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(p, format!("font={family}\nfont_size={size}\n"));
    }
}

#[cfg(test)]
mod tests {
    /// Every Windows install has Arial with its bold face.
    #[test]
    fn lists_installed_fonts() {
        let all = super::installed();
        let arial = all.iter().find(|f| f.name == "Arial").expect("Arial");
        assert!(arial.bold.is_some());
        assert!(all.iter().all(|f| !f.name.ends_with(" Bold")));
    }
}

