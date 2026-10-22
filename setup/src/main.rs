//! etapix-setup: a per-user installer with Etapix built in.
//!
//! - Installs to `%LOCALAPPDATA%\Programs\etapix` (no admin rights).
//! - Adds a Start Menu shortcut and an entry in Windows' "Installed apps" (current user only),
//!   whose uninstall runs this same program with `/uninstall`.
//! - Leaves Microsoft Project and its file associations alone: Etapix opens and saves Project XML
//!   files only when you ask it to.
//!
//! `/silent` skips every dialog (for IT deployment and tests): `etapix-setup.exe /silent`,
//! `desinstalar.exe /uninstall /silent`.
//!
//! Build `etapix` in release first; this program embeds `target/release/etapix.exe`.

#![windows_subsystem = "windows"]

use std::path::{Path, PathBuf};
use std::process::Command;

use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

const APP: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../target/release/etapix.exe"));
const VERSION: &str = env!("CARGO_PKG_VERSION");
const UNINSTALL_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\etapix";

fn install_dir() -> PathBuf {
    PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default()).join("Programs").join("etapix")
}

fn shortcut() -> PathBuf {
    PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
        .join(r"Microsoft\Windows\Start Menu\Programs\etapix.lnk")
}

fn silent() -> bool {
    std::env::args().any(|a| a.eq_ignore_ascii_case("/silent"))
}

fn ask(text: &str) -> bool {
    if silent() {
        return true;
    }
    MessageDialog::new()
        .set_title("Etapix")
        .set_description(text)
        .set_buttons(MessageButtons::YesNo)
        .show()
        == MessageDialogResult::Yes
}

fn tell(level: MessageLevel, text: &str) {
    if silent() {
        eprintln!("{text}");
        if matches!(level, MessageLevel::Error) {
            std::process::exit(1);
        }
        return;
    }
    MessageDialog::new().set_title("Etapix").set_level(level).set_description(text).show();
}

/// Runs a command without a console window and reports a non-zero exit.
fn run(program: &str, args: &[&str]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let status = Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| format!("{program}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{program} terminou com {status}")) }
}

fn create_shortcut(target: &Path, link: &Path) -> Result<(), String> {
    // WScript.Shell is part of every Windows install; paths are passed as arguments, not code.
    let script = "param($t,$l) $s=(New-Object -ComObject WScript.Shell).CreateShortcut($l); \
                  $s.TargetPath=$t; $s.WorkingDirectory=(Split-Path $t); $s.Save()";
    run(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &format!("& {{{script}}} '{}' '{}'", target.display(), link.display())],
    )
}

fn register(dir: &Path, uninstaller: &Path) -> Result<(), String> {
    let size_kb = (APP.len() / 1024).to_string();
    let uninstall = format!("\"{}\" /uninstall", uninstaller.display());
    let icon = dir.join("etapix.exe").display().to_string();
    let location = dir.display().to_string();
    for (name, kind, value) in [
        ("DisplayName", "REG_SZ", "Etapix"),
        ("DisplayVersion", "REG_SZ", VERSION),
        ("Publisher", "REG_SZ", "Equipe (uso interno)"),
        ("InstallLocation", "REG_SZ", location.as_str()),
        ("DisplayIcon", "REG_SZ", icon.as_str()),
        ("UninstallString", "REG_SZ", uninstall.as_str()),
        ("EstimatedSize", "REG_DWORD", size_kb.as_str()),
        ("NoModify", "REG_DWORD", "1"),
        ("NoRepair", "REG_DWORD", "1"),
    ] {
        run("reg", &["add", UNINSTALL_KEY, "/v", name, "/t", kind, "/d", value, "/f"])?;
    }
    Ok(())
}

fn install() -> Result<PathBuf, String> {
    let dir = install_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let exe = dir.join("etapix.exe");
    std::fs::write(&exe, APP).map_err(|e| format!("{}: {e} (o Etapix está aberto?)", exe.display()))?;
    // Keep a copy of this installer as the uninstaller.
    let uninstaller = dir.join("desinstalar.exe");
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    if me != uninstaller {
        std::fs::copy(&me, &uninstaller).map_err(|e| format!("{}: {e}", uninstaller.display()))?;
    }
    create_shortcut(&exe, &shortcut())?;
    register(&dir, &uninstaller)?;
    Ok(exe)
}

fn uninstall() -> Result<(), String> {
    let _ = std::fs::remove_file(shortcut());
    let _ = run("reg", &["delete", UNINSTALL_KEY, "/f"]);
    let dir = install_dir();
    let _ = std::fs::remove_file(dir.join("etapix.exe"));
    // This program is running from the folder: a detached shell retries deleting the folder
    // (up to ~20 s) until this process has exited and released the file.
    let d = dir.display();
    let cleanup = format!(
        "for /l %i in (1,1,10) do (ping -n 3 127.0.0.1 > nul & rmdir /s /q \"{d}\" & if not exist \"{d}\" exit)"
    );
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    // raw_arg: cmd does not understand the \" escaping that `args` would apply to the quotes.
    // Break away from any job object so closing this program does not end the cleanup.
    let spawn = |flags| Command::new("cmd").raw_arg(format!("/c {cleanup}")).creation_flags(flags).spawn();
    spawn(CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(CREATE_NO_WINDOW))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    if std::env::args().any(|a| a.eq_ignore_ascii_case("/uninstall")) {
        if ask("Remover o Etapix deste computador?\n\nSeus arquivos de projeto não são apagados.") {
            match uninstall() {
                Ok(()) => tell(MessageLevel::Info, "Etapix foi removido."),
                Err(e) => tell(MessageLevel::Error, &format!("Não foi possível remover tudo: {e}")),
            }
        }
        return;
    }
    let question = format!(
        "Instalar o Etapix {VERSION} em\n{}?\n\nInstalação só para o seu usuário, sem administrador. O Microsoft Project \
         instalado não é alterado.",
        install_dir().display()
    );
    if !ask(&question) {
        return;
    }
    match install() {
        Ok(exe) => {
            if !silent() && ask("Etapix instalado. Ele está no Menu Iniciar.\n\nAbrir agora?") {
                let _ = Command::new(exe).spawn();
            }
        }
        Err(e) => tell(MessageLevel::Error, &format!("A instalação falhou: {e}")),
    }
}
