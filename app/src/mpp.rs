//! `.mpp` files.
//!
//! - Reading is native ([`pjsched::mpp::mpp_to_mspdi`]): no Microsoft Project needed, on Windows
//!   or Linux. Only if the native reader cannot make sense of a file does Etapix fall back to the
//!   installed Project (Windows).
//! - Writing still goes through the installed Project: it opens Etapix's Project XML through its
//!   documented COM object model, invisibly, and saves it as `.mpp`. Saving goes to a temporary
//!   `.mpp` first and replaces the target only on success, so a failed conversion never damages
//!   the existing file.
//! - If Project is already open, the conversion runs in that instance (COM attaches to it) and
//!   only closes the file it opened; otherwise it starts a hidden instance and quits it.
//! - Project adds every file it opens to its recent list; the temporary XML files are removed
//!   from that list afterwards.

#![cfg_attr(not(windows), allow(dead_code, unused_imports))]

use std::path::{Path, PathBuf};
use std::process::Command;

const SCRIPT: &str = r#"
param([string]$Src, [string]$Dst, [string]$Mode, [string]$Temp)
$ErrorActionPreference = 'Stop'
# Errors go back to Etapix as UTF-8 text (the console code page would garble accents).
[Console]::OutputEncoding = [Text.Encoding]::UTF8
trap { [Console]::Error.WriteLine($_.Exception.Message); exit 2 }
$M = [System.Reflection.Missing]::Value
$running = @(Get-Process WINPROJ -ErrorAction SilentlyContinue).Count -gt 0
try {
  # ETAPIX_PROJECT_PROGID lets a test stand in for a machine without Project.
  $progid = if ($env:ETAPIX_PROJECT_PROGID) { $env:ETAPIX_PROJECT_PROGID } else { 'MSProject.Application' }
  $app = New-Object -ComObject $progid
} catch {
  # Project is not installed, or not registered for automation in this (32/64-bit) view.
  [Console]::Error.WriteLine($_.Exception.Message)
  exit 3
}
$alerts = $app.DisplayAlerts
$app.DisplayAlerts = $false
try {
  # FileOpenEx(Name, ReadOnly, Merge, TaskInformation, Table, Sheet, NoAuto, UserID,
  #            DatabasePassWord, FormatID, Map, openPool=pjDoNotOpenPool, ...)
  $format = if ($Mode -eq 'to-mpp') { 'MSProject.XML' } else { $M }
  $open = @($Src, $true) + @($M) * 7 + @($format, $M, 4) + @($M) * 5
  [void]$app.FileOpenEx.Invoke($open)
  # FileSaveAs(Name, Format: 0 = pjMPP, ..., FormatID)
  $saveFormat = if ($Mode -eq 'to-mpp') { $M } else { 'MSProject.XML' }
  $save = @($Dst, 0) + @($M) * 7 + @($saveFormat) + @($M) * 9
  [void]$app.FileSaveAs.Invoke($save)
  [void]$app.FileCloseEx(0, $M, $M)
} finally {
  $app.DisplayAlerts = $alerts
  if (-not $running) { [void]$app.Quit(0) }
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
# Drop etapix's temporary files from Project's recent lists.
$root = 'HKCU:\Software\Microsoft\Office\16.0\MS Project\User MRU'
if (Test-Path $root) {
  foreach ($list in Get-ChildItem $root -Recurse | Where-Object { $_.PSChildName -in 'File MRU', 'Place MRU' }) {
    $names = $list.GetValueNames() | Where-Object { $_ -match '^Item (\d+)$' } | Sort-Object { [int]($_ -replace '\D', '') }
    $keep = foreach ($n in $names) {
      $v = $list.GetValue($n)
      if ($v -notlike "*$Temp*") { [pscustomobject]@{ Item = $v; Meta = $list.GetValue(($n -replace 'Item ', 'Item Metadata ')) } }
    }
    if (@($keep).Count -eq @($names).Count) { continue }
    foreach ($nm in ($list.GetValueNames() | Where-Object { $_ -match '^Item (Metadata )?\d+$' })) { Remove-ItemProperty -Path $list.PSPath -Name $nm }
    $i = 1
    foreach ($e in $keep) {
      New-ItemProperty -Path $list.PSPath -Name "Item $i" -Value $e.Item -PropertyType String | Out-Null
      if ($null -ne $e.Meta) { New-ItemProperty -Path $list.PSPath -Name "Item Metadata $i" -Value $e.Meta -PropertyType String | Out-Null }
      $i++
    }
  }
}
"#;

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("etapix-mpp")
}

#[cfg(not(windows))]
fn convert(_: &Path, _: &Path, _: &str) -> Result<(), String> {
    Err("arquivos .mpp precisam do Microsoft Project, que só existe no Windows. Salve ou abra como \
         Project XML (.xml): o Project abre e salva esse formato."
        .into())
}

#[cfg(windows)]
fn convert(src: &Path, dst: &Path, mode: &str) -> Result<(), String> {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let script = dir.join("converter.ps1");
    // UTF-8 with BOM so Windows PowerShell 5.1 reads non-ASCII paths in the script correctly.
    std::fs::write(&script, [&[0xEF, 0xBB, 0xBF][..], SCRIPT.as_bytes()].concat()).map_err(|e| e.to_string())?;
    // A 32-bit Project may register its automation only for 32-bit programs: when the 64-bit
    // PowerShell cannot find it, the 32-bit one (SysWOW64) tries again.
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let shells = ["powershell".to_string(), format!(r"{windir}\SysWOW64\WindowsPowerShell\v1.0\powershell.exe")];
    let mut last = None;
    for shell in &shells {
        let out = run_script(shell, &script, src, dst, mode, &dir)?;
        if out.status.code() == Some(3) {
            last = Some(out);
            continue;
        }
        if !out.status.success() || !dst.exists() {
            let err = String::from_utf8_lossy(&out.stderr);
            let first = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("o Project não converteu o arquivo");
            return Err(format!("conversão pelo Microsoft Project falhou: {first}"));
        }
        return Ok(());
    }
    // The COM error code is enough to tell what happened (80040154: class not registered).
    let code = last
        .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
        .and_then(|e| e.split_whitespace().find(|w| w.len() == 8 && w.chars().all(|c| c.is_ascii_hexdigit())).map(String::from))
        .unwrap_or_else(|| "desconhecido".into());
    Err(format!(
        "o Microsoft Project não foi encontrado nesta máquina, e é ele que lê e grava arquivos .mpp. \
         Instale o Project para abrir .mpp aqui, ou peça o arquivo em Project XML (no Project: Arquivo > \
         Salvar como > Formato XML), que o Etapix abre sem o Project. (Código COM: {code})"
    ))
}

#[cfg(windows)]
fn run_script(shell: &str, script: &Path, src: &Path, dst: &Path, mode: &str, dir: &Path) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new(shell)
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script)
        .arg("-Src")
        .arg(src)
        .arg("-Dst")
        .arg(dst)
        .arg("-Mode")
        .arg(mode)
        .arg("-Temp")
        .arg(dir)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("não foi possível iniciar o PowerShell: {e}"))
}

fn temp_file(ext: &str) -> PathBuf {
    let _ = std::fs::create_dir_all(temp_dir());
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    temp_dir().join(format!("{}-{stamp}.{ext}", std::process::id()))
}

/// Reads a `.mpp` file as Project XML text: natively, or through Project when the native reader
/// cannot (and Project is there to ask).
pub fn read_mpp(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    match pjsched::mpp::mpp_to_mspdi(&bytes) {
        Ok(xml) => Ok(xml),
        Err(native) => read_through_project(path).map_err(|_| format!("não foi possível ler o .mpp: {native}")),
    }
}

fn read_through_project(path: &Path) -> Result<String, String> {
    let xml = temp_file("xml");
    convert(path, &xml, "to-xml")?;
    let text = std::fs::read_to_string(&xml).map_err(|e| e.to_string());
    let _ = std::fs::remove_file(&xml);
    text
}

/// Writes Project XML text as a `.mpp` file, replacing `path` only once the conversion worked.
pub fn write_mpp(path: &Path, xml: &str) -> Result<(), String> {
    let src = temp_file("xml");
    let out = temp_file("mpp");
    std::fs::write(&src, xml).map_err(|e| e.to_string())?;
    let result = convert(&src, &out, "to-mpp").and_then(|()| {
        std::fs::copy(&out, path).map(|_| ()).map_err(|e| format!("{}: {e} (o arquivo está aberto no Project?)", path.display()))
    });
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&out);
    result
}

pub fn is_mpp(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("mpp"))
}

#[cfg(all(test, windows))]
mod tests {
    /// On a machine without Project, .mpp files still open (natively); the conversion through
    /// Project, used for saving, says plainly that Project is missing.
    #[test]
    fn missing_project_gives_a_clear_message() {
        std::env::set_var("ETAPIX_PROJECT_PROGID", "Etapix.ProjetoInexistente");
        let dir = std::env::temp_dir().join("etapix-mpp-test");
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("teste.mpp");
        std::fs::write(&fake, b"x").unwrap();
        let err = super::read_through_project(&fake).unwrap_err();
        // A real .mpp still opens: the native reader does not need Project.
        let real = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/mpp-corpus/b1/S1-caminho-critico.mpp");
        let xml = super::read_mpp(&real);
        std::env::remove_var("ETAPIX_PROJECT_PROGID");
        assert!(xml.is_ok_and(|x| x.contains("<Name>A</Name>")));
        assert!(err.contains("Microsoft Project não foi encontrado"), "{err}");
        assert!(err.contains("Formato XML"), "{err}");
        assert!(!err.contains('\u{FFFD}'), "garbled text: {err}");
        eprintln!("{err}");
    }
}
