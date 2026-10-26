use std::path::Path;
use windows::core::{Interface, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\etapix";
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().chain(Some(0)).collect()
}
fn text(value: &str) -> Vec<u16> {
    wide(std::ffi::OsStr::new(value))
}

pub fn create_shortcut(target: &Path, link: &Path) -> Result<(), String> {
    let target_w = wide(target.as_os_str());
    let parent = target.parent().ok_or("Executável sem pasta")?;
    let parent_w = wide(parent.as_os_str());
    let link_w = wide(link.as_os_str());
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| e.to_string())?;
        let result = (|| -> windows::core::Result<()> {
            let shell: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            shell.SetPath(PCWSTR(target_w.as_ptr()))?;
            shell.SetWorkingDirectory(PCWSTR(parent_w.as_ptr()))?;
            let persist: IPersistFile = shell.cast()?;
            persist.Save(PCWSTR(link_w.as_ptr()), true)?;
            Ok(())
        })();
        CoUninitialize();
        result.map_err(|e| e.to_string())
    }
}

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
fn set(key: &Key, name: &str, kind: REG_VALUE_TYPE, data: &[u8]) -> Result<(), String> {
    let name = text(name);
    unsafe {
        RegSetValueExW(key.0, PCWSTR(name.as_ptr()), 0, kind, Some(data))
            .ok()
            .map_err(|e| e.to_string())
    }
}
pub fn register(dir: &Path, uninstaller: &Path, version: &str, size_kb: u32) -> Result<(), String> {
    let name = text(KEY);
    let mut key = HKEY::default();
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(name.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        )
        .ok()
        .map_err(|e| e.to_string())?;
    }
    let key = Key(key);
    let uninstall = format!("\"{}\" /uninstall", uninstaller.display());
    let icon = dir.join("etapix.exe").display().to_string();
    let location = dir.display().to_string();
    for (name, value) in [
        ("DisplayName", "Etapix"),
        ("DisplayVersion", version),
        ("Publisher", "Pontesrrafael / Etapix"),
        ("InstallLocation", &location),
        ("DisplayIcon", &icon),
        ("UninstallString", &uninstall),
    ] {
        let bytes: Vec<u8> = text(value).iter().flat_map(|v| v.to_le_bytes()).collect();
        set(&key, name, REG_SZ, &bytes)?;
    }
    for (name, value) in [("EstimatedSize", size_kb), ("NoModify", 1), ("NoRepair", 1)] {
        set(&key, name, REG_DWORD, &value.to_le_bytes())?;
    }
    Ok(())
}
pub fn unregister() -> Result<(), String> {
    let name = text(KEY);
    let result = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(name.as_ptr())) };
    if result == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    result.ok().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_shortcut_round_trip() {
        use windows::Win32::System::Com::STGM_READ;
        use windows::Win32::UI::Shell::SLGP_RAWPATH;
        let dir = std::env::temp_dir().join(format!("etapix-shortcut-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("D'Avila & TI.exe");
        let link = dir.join("Etapix.lnk");
        create_shortcut(&target, &link).unwrap();
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
            {
                let shell: IShellLinkW =
                    CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
                let persist: IPersistFile = shell.cast().unwrap();
                let path = wide(link.as_os_str());
                persist.Load(PCWSTR(path.as_ptr()), STGM_READ).unwrap();
                let mut buffer = [0u16; 1024];
                shell
                    .GetPath(&mut buffer, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)
                    .unwrap();
                let end = buffer.iter().position(|v| *v == 0).unwrap();
                assert_eq!(
                    String::from_utf16(&buffer[..end]).unwrap(),
                    target.to_str().unwrap()
                );
            }
            CoUninitialize();
        }
        std::fs::remove_file(&link).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
    #[test]
    fn paths_are_utf16_data_and_preserve_apostrophes() {
        let path = std::ffi::OsStr::new("C:\\Users\\D'Ávila & TI\\etapix.exe");
        let value = wide(path);
        assert_eq!(value.last(), Some(&0));
        assert_eq!(
            String::from_utf16(&value[..value.len() - 1]).unwrap(),
            path.to_str().unwrap()
        );
    }
}
