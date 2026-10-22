//! `.mpp` files through the Microsoft Project installed on the machine (Windows only: on Linux
//! there is no Project, and `.mpp` files are refused with a message pointing to Project XML).
//!
//! The `.mpp` format is binary and undocumented, so Etapix does not parse it: it asks Project
//! (through its documented COM object model, invisibly) to convert between `.mpp` and Project
//! XML, which Etapix reads and writes itself.
//!
//! - If Project is already open, the conversion runs in that instance (COM attaches to it) and
//!   only closes the file it opened; otherwise it starts a hidden instance and quits it.
//! - Saving goes to a temporary `.mpp` first and replaces the target only on success, so a
//!   failed conversion never damages the existing file.
//! - Project adds every file it opens to its recent list; the temporary XML files are removed
//!   from that list afterwards (the `.mpp` itself stays, as when you open it in Project).

#![cfg_attr(not(windows), allow(dead_code, unused_imports))]

use std::path::{Path, PathBuf};
use std::process::Command;

const SCRIPT: &str = r#"
param([string]$Src, [string]$Dst, [string]$Mode, [string]$Temp)
$ErrorActionPreference = 'Stop'
$M = [System.Reflection.Missing]::Value
$running = @(Get-Process WINPROJ -ErrorAction SilentlyContinue).Count -gt 0
$app = New-Object -ComObject MSProject.Application
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
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .arg("-Src")
        .arg(src)
        .arg("-Dst")
        .arg(dst)
        .arg("-Mode")
        .arg(mode)
        .arg("-Temp")
        .arg(&dir)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("não foi possível iniciar o PowerShell: {e}"))?;
    if !out.status.success() || !dst.exists() {
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("o Project não converteu o arquivo");
        return Err(format!("conversão pelo Microsoft Project falhou: {first}"));
    }
    Ok(())
}

fn temp_file(ext: &str) -> PathBuf {
    let _ = std::fs::create_dir_all(temp_dir());
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    temp_dir().join(format!("{}-{stamp}.{ext}", std::process::id()))
}

/// Reads a `.mpp` file as Project XML text.
pub fn read_mpp(path: &Path) -> Result<String, String> {
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
