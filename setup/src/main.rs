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
//! It also installs the Etapix connector for Claude and Codex (`etapix-mcp.exe`, which registers
//! itself in Claude Code and Codex and offers the `.mcpb` extension to Claude Desktop).
//!
//! Build with `toolsuild-installer.ps1`: this program embeds `target/release/etapix.exe`,
//! `target/release/etapix-mcp.exe` and `dist/etapix.mcpb`.

#![windows_subsystem = "windows"]

use std::path::{Path, PathBuf};
use std::process::Command;

use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

const APP: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../target/release/etapix.exe"
));
const MCP: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../target/release/etapix-mcp.exe"
));
const MCPB: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../dist/etapix.mcpb"));
/// The licenses travel with the programs: Etapix's own and the notices of the libraries and
/// fonts built into it (`tools/third_party_notices.py`).
const LICENSES: [(&str, &[u8]); 3] = [
    (
        "LICENSE-MIT.txt",
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../LICENSE-MIT")),
    ),
    (
        "LICENSE-APACHE.txt",
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../LICENSE-APACHE")),
    ),
    (
        "THIRD-PARTY-NOTICES.txt",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../THIRD-PARTY-NOTICES.txt"
        )),
    ),
];
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn install_dir() -> PathBuf {
    PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default())
        .join("Programs")
        .join("etapix")
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
    MessageDialog::new()
        .set_title("Etapix")
        .set_level(level)
        .set_description(text)
        .show();
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
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} terminou com {status}"))
    }
}

mod windows_install;

fn create_shortcut(target: &Path, link: &Path) -> Result<(), String> {
    windows_install::create_shortcut(target, link)
}

fn register(dir: &Path, uninstaller: &Path) -> Result<(), String> {
    windows_install::register(dir, uninstaller, VERSION, (APP.len() / 1024) as u32)
}

/// Writes `dir/name`. Claude and Codex keep the connector running while they are open, and
/// Windows refuses to overwrite a running program but lets it be renamed: the old copy moves
/// aside and is deleted on a later install, once nothing runs it.
fn write_in_use(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_name()
                .to_string_lossy()
                .starts_with(&format!("{name}.antigo"))
            {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let target = dir.join(name);
    if std::fs::write(&target, bytes).is_ok() {
        return Ok(());
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let aside = dir.join(format!("{name}.antigo-{stamp}"));
    std::fs::rename(&target, &aside).map_err(|e| format!("{name}: {e}"))?;
    std::fs::write(&target, bytes).map_err(|e| format!("{name}: {e}"))
}

fn install() -> Result<PathBuf, String> {
    let dir = install_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    remove_set_aside(&dir);
    let exe = dir.join("etapix.exe");
    std::fs::write(&exe, APP)
        .map_err(|e| format!("{}: {e} (o Etapix está aberto?)", exe.display()))?;
    // Keep a copy of this installer as the uninstaller.
    let uninstaller = dir.join("desinstalar.exe");
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    if me != uninstaller {
        std::fs::copy(&me, &uninstaller).map_err(|e| format!("{}: {e}", uninstaller.display()))?;
    }
    for (name, bytes) in [("etapix-mcp.exe", MCP), ("etapix.mcpb", MCPB)] {
        write_in_use(&dir, name, bytes)?;
    }
    for (name, bytes) in LICENSES {
        std::fs::write(dir.join(name), bytes).map_err(|e| format!("{name}: {e}"))?;
    }
    create_shortcut(&exe, &shortcut())?;
    register(&dir, &uninstaller)?;
    Ok(exe)
}

/// Deletes the copies `write_in_use` set aside in earlier installs; one still running stays.
fn remove_set_aside(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        if e.file_name().to_string_lossy().contains(".antigo-") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

fn uninstall() -> Result<(), String> {
    let dir = install_dir();
    // Never recursively delete the installation directory: it may contain user files.
    let mcp = dir.join("etapix-mcp.exe");
    if mcp.exists() {
        run(&mcp.display().to_string(), &["remover"])?;
    }
    let remove = |path: &Path| -> Result<(), String> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    };
    for name in [
        "etapix.exe",
        "etapix-mcp.exe",
        "etapix.mcpb",
        "LICENSE-MIT.txt",
        "LICENSE-APACHE.txt",
        "THIRD-PARTY-NOTICES.txt",
    ] {
        remove(&dir.join(name))?;
    }
    remove(&shortcut())?;
    windows_install::unregister()?;
    // Windows locks the running uninstaller. Keep it rather than spawning a shell
    // to delete itself; the completion message documents this residual file.
    Ok(())
}

fn main() {
    if std::env::args().any(|a| a.eq_ignore_ascii_case("/uninstall")) {
        if ask("Remover o Etapix deste computador?\n\nSeus arquivos de projeto não são apagados.")
        {
            match uninstall() {
                Ok(()) => tell(MessageLevel::Info, "Etapix foi removido. O arquivo desinstalar.exe permanece na pasta de instalação e pode ser excluído após fechar esta janela. Arquivos desconhecidos foram preservados."),
                Err(e) => tell(MessageLevel::Error, &format!("Não foi possível remover tudo: {e}")),
            }
        }
        return;
    }
    let question = format!(
        "Instalar o Etapix {VERSION} em\n{}?\n\nInstalação só para o seu usuário, sem administrador. O Microsoft Project \
         instalado não é alterado.

Inclui o conector do Etapix para o Claude e o Codex.",
        install_dir().display()
    );
    if !ask(&question) {
        return;
    }
    match install() {
        Ok(exe) => {
            // The connector registers itself; in silent mode Claude Desktop's dialog is skipped.
            let mcp = install_dir().join("etapix-mcp.exe");
            let mcpb = install_dir().join("etapix.mcpb");
            let mut args = vec!["instalar".to_string()];
            if !silent() {
                args.push(mcpb.display().to_string());
            }
            if let Err(e) = run(
                &mcp.display().to_string(),
                &args.iter().map(String::as_str).collect::<Vec<_>>(),
            ) {
                tell(
                    MessageLevel::Warning,
                    &format!("O Etapix foi instalado, mas o conector do Claude/Codex não: {e}"),
                );
            }
            if !silent() && ask("Etapix instalado. Ele está no Menu Iniciar.\n\nAbrir agora?") {
                let _ = Command::new(exe).spawn();
            }
        }
        Err(e) => tell(MessageLevel::Error, &format!("A instalação falhou: {e}")),
    }
}
