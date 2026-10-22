//! Fonts installed on this machine: on Windows from the registry where Windows lists them (for
//! all users and for this user), on Linux from fontconfig (`fc-list`). They are loaded from the
//! machine's own font files when picked, never copied into Etapix.

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

/// Fontconfig's faces as (display name in the registry's style, "Family" or "Family Bold").
#[cfg(not(windows))]
fn entries() -> Vec<(String, PathBuf)> {
    let Ok(o) = std::process::Command::new("fc-list").args([":", "family", "style", "file"]).output() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // Lines look like "/usr/share/fonts/.../DejaVuSans-Bold.ttf: DejaVu Sans:style=Bold".
    for line in String::from_utf8_lossy(&o.stdout).lines() {
        let mut parts = line.splitn(3, ':');
        let (Some(file), Some(family), style) = (parts.next(), parts.next(), parts.next()) else { continue };
        let lower = file.to_lowercase();
        if !(lower.ends_with(".ttf") || lower.ends_with(".otf") || lower.ends_with(".ttc")) {
            continue;
        }
        let family = family.split(',').next().unwrap_or("").trim().to_string();
        let style = style.and_then(|s| s.strip_prefix("style=")).unwrap_or("Regular");
        let style = style.split(',').next().unwrap_or("Regular").trim();
        let name = match style {
            "Regular" | "Book" | "Roman" | "Normal" => family,
            other => format!("{family} {other}"),
        };
        out.push((name, PathBuf::from(file.trim())));
    }
    out
}

/// Registry entries as (display name, file path).
#[cfg(windows)]
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

/// The settings file (`key=value` lines).
fn settings_path() -> Option<PathBuf> {
    // Tests drive the real interface: they must never touch the person's own settings.
    if cfg!(test) {
        return Some(std::env::temp_dir().join(format!("etapix-test-settings-{}.txt", std::process::id())));
    }
    // %APPDATA%\Etapix on Windows; $XDG_CONFIG_HOME/etapix (usually ~/.config/etapix) on Linux.
    let dir = if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Etapix"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|c| c.join("etapix"))
    };
    dir.map(|d| d.join("settings.txt"))
}

/// All saved settings, in file order.
fn settings() -> Vec<(String, String)> {
    let text = settings_path().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    text.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect()
}

/// A saved setting.
pub fn setting(key: &str) -> Option<String> {
    settings().into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Saves settings, keeping the others already in the file.
pub fn save_settings(values: &[(&str, String)]) {
    let Some(p) = settings_path() else { return };
    let mut all = settings();
    for (k, v) in values {
        match all.iter_mut().find(|(key, _)| key == k) {
            Some(e) => e.1 = v.clone(),
            None => all.push((k.to_string(), v.clone())),
        }
    }
    let _ = std::fs::create_dir_all(p.parent().unwrap());
    let _ = std::fs::write(p, all.iter().map(|(k, v)| format!("{k}={v}\n")).collect::<String>());
}

/// The saved font choice: family name (empty for the brand font) and text size.
pub fn load_choice() -> (String, f32) {
    (setting("font").unwrap_or_default(), setting("font_size").and_then(|v| v.parse().ok()).unwrap_or(12.0))
}

pub fn save_choice(family: &str, size: f32) {
    save_settings(&[("font", family.to_string()), ("font_size", size.to_string())]);
}

#[cfg(all(test, windows))]
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

