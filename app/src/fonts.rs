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
    /// The installed family that stands in for this one (see `STAND_INS`).
    pub stand_in: Option<String>,
}

impl Family {
    /// The name in the font list: "Calibri (Carlito)" when another family stands in.
    pub fn label(&self) -> String {
        match &self.stand_in {
            Some(real) => format!("{} ({real})", self.name),
            None => self.name.clone(),
        }
    }
}

/// Windows fonts a machine may lack (Linux has none of them) and the free families that can
/// stand in, best first. The first three of each of Times New Roman, Arial, Courier New, Calibri
/// and Cambria have the same letter widths, so text wraps and columns fit the same; Tahoma and
/// Verdana have no free twin and get the closest shapes. Etapix never ships Microsoft's fonts.
const STAND_INS: [(&str, &[&str]); 7] = [
    ("Arial", &["Liberation Sans", "Arimo", "Nimbus Sans"]),
    ("Calibri", &["Carlito"]),
    ("Cambria", &["Caladea"]),
    ("Courier New", &["Liberation Mono", "Cousine", "Nimbus Mono PS"]),
    ("Tahoma", &["DejaVu Sans Condensed", "DejaVu Sans", "Noto Sans"]),
    ("Times New Roman", &["Liberation Serif", "Tinos", "Nimbus Roman"]),
    ("Verdana", &["DejaVu Sans", "Noto Sans"]),
];

/// Adds each missing Windows font whose stand-in is installed, under the Windows name.
fn with_stand_ins(mut families: Vec<Family>) -> Vec<Family> {
    let find = |all: &[Family], name: &str| all.iter().find(|f| f.name.eq_ignore_ascii_case(name)).cloned();
    for (name, candidates) in STAND_INS {
        if find(&families, name).is_some() {
            continue;
        }
        if let Some(real) = candidates.iter().find_map(|c| find(&families, c)) {
            families.push(Family { name: name.to_string(), stand_in: Some(real.name.clone()), ..real });
        }
    }
    families
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
            Family { name: family, regular: path.clone(), bold, stand_in: None }
        })
        .filter(|f| f.regular.exists())
        .collect();
    families.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    let mut families = with_stand_ins(families);
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

#[cfg(test)]
mod stand_in_tests {
    use super::*;

    fn family(name: &str) -> Family {
        Family { name: name.into(), regular: PathBuf::from(format!("{name}.ttf")), bold: Some(PathBuf::from(format!("{name}-Bold.ttf"))), stand_in: None }
    }

    #[test]
    fn missing_windows_fonts_get_an_installed_stand_in() {
        let all = with_stand_ins(vec![family("Carlito"), family("DejaVu Sans"), family("Liberation Serif"), family("Arial")]);
        let get = |n: &str| all.iter().find(|f| f.name == n);
        let calibri = get("Calibri").expect("Calibri");
        assert_eq!(calibri.label(), "Calibri (Carlito)");
        assert_eq!(calibri.regular, PathBuf::from("Carlito.ttf"));
        assert_eq!(calibri.bold, Some(PathBuf::from("Carlito-Bold.ttf")));
        assert_eq!(get("Times New Roman").unwrap().stand_in.as_deref(), Some("Liberation Serif"));
        // The condensed face is not here: Tahoma takes the next candidate.
        assert_eq!(get("Tahoma").unwrap().stand_in.as_deref(), Some("DejaVu Sans"));
        // An installed font is never replaced, and one with no stand-in installed is not listed.
        assert_eq!(get("Arial").unwrap().label(), "Arial");
        assert!(get("Courier New").is_none() && get("Cambria").is_none());
    }
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

