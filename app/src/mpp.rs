//! `.mpp` files.
//!
//! - Reading is native ([`pjsched::mpp::mpp_to_mspdi`]): no Microsoft Project needed, on Windows
//!   or Linux. Only if the native reader cannot make sense of a file does Etapix fall back to the
//!   installed Project (Windows).
//! - Writing is native too. Edits to a project opened from a `.mpp`, tasks, links and
//!   assignments added or removed included, go into that file's own tables
//!   ([`pjsched::mpp::save_mpp`]); a new file is made from Etapix's template
//!   ([`pjsched::mpp::create_mpp`]). What the native writer cannot save yet goes through the
//!   installed Project, when there is one: it opens Etapix's Project XML through its documented
//!   COM object model, invisibly, and saves it as `.mpp`. Without Project, the file is rebuilt
//!   from the template, and the user is told what that leaves out.
//! - Either way the new file is written next to the target first and replaces it only on
//!   success, so a failed save never damages the existing file.
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
    Err("o Microsoft Project, a alternativa para este arquivo, não existe no Linux. Salve ou abra como \
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

/// Writes Project XML text as a `.mpp` file, replacing `path` only once the save worked. In order:
/// natively into the existing file (`path`, or `source`, the `.mpp` the project was opened from,
/// when `path` is new); through the installed Project, which keeps everything else in the file;
/// and natively from the template, which keeps every field Etapix shows but not Project's own
/// views and formats. Returns a notice for the user when it had to rebuild the file.
pub fn write_mpp(path: &Path, xml: &str, source: Option<&Path>) -> Result<Option<String>, String> {
    let base = if path.exists() { Some(path) } else { source.filter(|s| is_mpp(s) && s.exists()) };
    let Some(base) = base else {
        // A new file: the template is the native way, Project the fallback.
        return write_native(path, pjsched::mpp::create_mpp(xml)).or_else(|why| through_project_or(path, xml, &why)).map(|()| None);
    };
    let original = std::fs::read(base).map_err(|e| format!("{}: {e}", base.display()))?;
    let Err(why) = write_native(path, pjsched::mpp::save_mpp(&original, xml)) else { return Ok(None) };
    match write_through_project(path, xml) {
        Ok(()) => Ok(None),
        Err(com) if !project_missing(&com) => Err(com),
        Err(_) => write_native(path, pjsched::mpp::create_mpp(xml)).map(|()| Some(REBUILT.into())).map_err(|rebuild| missing_project(&format!("{why}; {rebuild}"))),
    }
}

const REBUILT: &str = "O .mpp foi salvo, recriado pelo Etapix: o Microsoft Project, que gravaria essa alteração no \
                       arquivo original, não está disponível nesta máquina. Tarefas, vínculos, recursos, atribuições e \
                       calendários estão no arquivo; modos de exibição e formatações feitos no Project, não.";

fn project_missing(com: &str) -> bool {
    com.contains("não foi encontrado") || cfg!(not(windows))
}

fn missing_project(why: &str) -> String {
    format!(
        "não foi possível gravar direto no .mpp ({why}), e o Microsoft Project, que gravaria essa alteração, \
         não está disponível nesta máquina. Salve em Project XML (.xml): o Project abre esse formato."
    )
}

/// Saves through Project, or explains what the native writer could not do when Project is missing.
fn through_project_or(path: &Path, xml: &str, why: &str) -> Result<(), String> {
    write_through_project(path, xml).map_err(|com| if project_missing(&com) { missing_project(why) } else { com })
}

/// Writes the `.mpp` the native writer made as `path`, replacing it only once it is complete.
fn write_native(path: &Path, made: Result<Vec<u8>, pjsched::mpp::MppError>) -> Result<(), String> {
    let bytes = made.map_err(|e| e.0)?;
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let partial = path.with_file_name(format!(".{file_name}.etapix-tmp"));
    std::fs::write(&partial, bytes).map_err(|e| format!("{}: {e}", partial.display()))?;
    std::fs::rename(&partial, path).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        format!("{}: {e} (o arquivo está aberto no Project?)", path.display())
    })
}

fn write_through_project(path: &Path, xml: &str) -> Result<(), String> {
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
    /// The tests that stand in for a missing Project share one environment variable.
    static NO_PROJECT: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// On a machine without Project, .mpp files still open (natively); the conversion through
    /// Project, used for saving, says plainly that Project is missing.
    #[test]
    fn missing_project_gives_a_clear_message() {
        let _only = NO_PROJECT.lock().unwrap_or_else(|e| e.into_inner());
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

    /// Without Project: edits, and a new task, save natively into the `.mpp` itself, with no notice
    /// (nothing of the file is lost); a new project saves as a new file.
    #[test]
    fn saves_without_project() {
        let _only = NO_PROJECT.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("ETAPIX_PROJECT_PROGID", "Etapix.ProjetoInexistente");
        let dir = std::env::temp_dir().join("etapix-mpp-save-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("copia.mpp");
        std::fs::copy(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/mpp-corpus/b1/S1-caminho-critico.mpp"), &file).unwrap();
        let mut doc = crate::doc::Doc::open(&file).unwrap();
        doc.apply(|p| {
            p.tasks[0].name = "A editada".into();
            Ok(())
        })
        .unwrap();
        let edited = doc.save(&file);
        let edited_xml = super::read_mpp(&file);
        doc.apply(|p| {
            p.tasks.push(pjsched::model::Task::new(99, 99, "Nova", 480));
            Ok(())
        })
        .unwrap();
        let added = doc.save(&file);
        let added_xml = super::read_mpp(&file);
        let new_file = dir.join("novo.mpp");
        let _ = std::fs::remove_file(&new_file);
        let created = crate::doc::Doc::blank().save(&new_file);
        std::env::remove_var("ETAPIX_PROJECT_PROGID");
        assert_eq!(edited, Ok(None));
        assert!(edited_xml.unwrap().contains("<Name>A editada</Name>"));
        assert_eq!(added, Ok(None));
        assert!(added_xml.unwrap().contains("<Name>Nova</Name>"));
        assert_eq!(created, Ok(None));
        assert!(super::read_mpp(&new_file).is_ok());
    }

    /// A user's session on a .mpp without Project: every kind of change the app makes is saved
    /// into the file itself and opens again as it was saved, with no notice.
    #[test]
    fn whole_session_saves_without_project() {
        let _only = NO_PROJECT.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("ETAPIX_PROJECT_PROGID", "Etapix.ProjetoInexistente");
        let dir = std::env::temp_dir().join("etapix-mpp-session-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("sessao.mpp");
        std::fs::copy(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/mpp-corpus/b1/S8-unidades-50.mpp"), &file).unwrap();
        let mut doc = crate::doc::Doc::open(&file).unwrap();
        let steps: Vec<Result<(), String>> = vec![
            doc.add_task("Montagem", 2.0).map(|_| ()),
            doc.set_predecessors(2, "1"),
            doc.add_resource("Equipe B"),
            doc.set_resources(2, "Equipe B"),
            doc.set_duration_days(0, 3.0),
            doc.set_percent(0, 50),
            doc.save_baseline(),
            doc.set_manual(1, true),
            doc.indent(2),
        ];
        let saved = doc.save(&file);
        let expected = super::read_mpp(&file).map(|_| doc.project.tasks.iter().map(|t| (t.uid, t.name.clone(), t.duration, t.progress.percent_complete, t.baseline.is_some(), t.manual.is_some(), t.parent)).collect::<Vec<_>>());
        let reopened = crate::doc::Doc::open(&file);
        std::env::remove_var("ETAPIX_PROJECT_PROGID");
        for (k, step) in steps.into_iter().enumerate() {
            step.unwrap_or_else(|e| panic!("step {k}: {e}"));
        }
        assert_eq!(saved, Ok(None));
        let reopened = reopened.unwrap();
        let got: Vec<_> = reopened.project.tasks.iter().map(|t| (t.uid, t.name.clone(), t.duration, t.progress.percent_complete, t.baseline.is_some(), t.manual.is_some(), t.parent)).collect();
        assert_eq!(expected.unwrap(), got);
        assert!(reopened.project.resources.iter().any(|r| r.name == "Equipe B"));
    }
}