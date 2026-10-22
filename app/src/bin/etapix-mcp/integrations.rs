//! `etapix-mcp instalar` / `etapix-mcp remover`: registers this server and the Etapix skill in
//! the assistants found for the current user (Claude Code, Codex, Claude Desktop), or takes them
//! out again. Used by the Etapix installer and by the standalone connector package.

use std::path::{Path, PathBuf};

const SKILL: &str = include_str!("../../../../integrations/skill/etapix/SKILL.md");

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default())
}

/// Writes `text` to `path` through a temporary file, keeping a one-time copy of the original.
fn replace(path: &Path, text: &str) -> Result<(), String> {
    let backup = path.with_extension(format!("{}.bak-etapix", path.extension().and_then(|e| e.to_str()).unwrap_or("")));
    if path.exists() && !backup.exists() {
        std::fs::copy(path, &backup).map_err(|e| format!("{}: {e}", backup.display()))?;
    }
    let tmp = path.with_extension("etapix-tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

fn put_skill(root: &Path) -> Result<(), String> {
    let dir = root.join("skills").join("etapix");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(dir.join("SKILL.md"), SKILL).map_err(|e| e.to_string())
}

fn drop_skill(root: &Path) {
    let _ = std::fs::remove_dir_all(root.join("skills").join("etapix"));
}

/// Claude Code keeps user-scope servers under `mcpServers` in `~/.claude.json`.
fn claude_code(exe: Option<&Path>) -> Result<bool, String> {
    let json = home().join(".claude.json");
    if !json.exists() && !home().join(".claude").exists() {
        return Ok(false);
    }
    let mut data: serde_json::Value = match std::fs::read_to_string(&json) {
        Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{}: {e}", json.display()))?,
        Err(_) => serde_json::json!({}),
    };
    let root = data.as_object_mut().ok_or("~/.claude.json não é um objeto JSON")?;
    let servers = root.entry("mcpServers").or_insert_with(|| serde_json::json!({}));
    let servers = servers.as_object_mut().ok_or("mcpServers não é um objeto")?;
    match exe {
        Some(exe) => {
            servers.insert(
                "etapix".into(),
                serde_json::json!({ "type": "stdio", "command": exe.display().to_string(), "args": [], "env": {} }),
            );
            put_skill(&home().join(".claude"))?;
        }
        None => {
            servers.remove("etapix");
            drop_skill(&home().join(".claude"));
        }
    }
    replace(&json, &serde_json::to_string_pretty(&data).unwrap())?;
    Ok(true)
}

/// Codex reads `[mcp_servers.<name>]` tables from `~/.codex/config.toml`.
fn codex(exe: Option<&Path>) -> Result<bool, String> {
    let dir = home().join(".codex");
    if !dir.exists() {
        return Ok(false);
    }
    let config = dir.join("config.toml");
    let text = std::fs::read_to_string(&config).unwrap_or_default();
    // Drop any existing [mcp_servers.etapix] table (up to the next table header).
    let mut kept = Vec::new();
    let mut skipping = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            skipping = t == "[mcp_servers.etapix]" || t.starts_with("[mcp_servers.etapix.");
        }
        if !skipping {
            kept.push(line);
        }
    }
    let mut out = kept.join("\n").trim_end().to_string();
    match exe {
        Some(exe) => {
            // A TOML literal string: backslashes are kept as they are.
            out.push_str(&format!("\n\n[mcp_servers.etapix]\ncommand = '{}'\nargs = []\n", exe.display()));
            put_skill(&dir)?;
        }
        None => {
            out.push('\n');
            drop_skill(&dir);
        }
    }
    replace(&config, out.trim_start())?;
    Ok(true)
}

/// Runs `instalar` or `remover`; prints what it did. `mcpb` is the Claude Desktop extension to
/// open (its own dialog asks the user to confirm).
pub fn run(install: bool, mcpb: Option<&Path>) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let target = install.then_some(exe.as_path());
    let mut done = Vec::new();
    if claude_code(target)? {
        done.push("Claude Code");
    }
    if codex(target)? {
        done.push("Codex");
    }
    let desktop = PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("Claude");
    if let (true, Some(mcpb), true) = (install, mcpb, desktop.exists()) {
        if mcpb.exists() {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let _ = std::process::Command::new("cmd")
                .args(["/c", "start", ""])
                .arg(mcpb)
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
            done.push("Claude Desktop (confirme em Instalar)");
        }
    }
    let verb = if install { "Etapix instalado em" } else { "Etapix removido de" };
    if done.is_empty() {
        println!("Nenhum Claude Code, Codex ou Claude Desktop encontrado para este usuário.");
    } else {
        println!("{verb}: {}.", done.join(", "));
    }
    Ok(())
}
