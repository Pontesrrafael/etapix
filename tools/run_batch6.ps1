param([string]$OutDir, [string]$Only = '')
# Batch 6: progress and actuals (% complete, actual start/finish, actual work) and rescheduling
# of unfinished work. CPM only; each case exports one `<case>-antes.xml`.
$ErrorActionPreference = 'Stop'
$M = [System.Reflection.Missing]::Value
$DAY = 480
$Start = [datetime]'2026-11-02 08:00'   # Monday
$FS = 1
$RESCHEDULE = 2   # UpdateProject action: reschedule uncompleted work after the update date

$app = New-Object -ComObject MSProject.Application
$app.Visible = $false
$app.DisplayAlerts = $false
"Project version: $($app.Version) build $($app.Build)"

function Add-Task($p, [string]$name, [int]$days) {
  $t = $p.Tasks.Add($name)
  $t.Manual = $false
  $t.Duration = $days * $DAY
  Write-Output -NoEnumerate $t
}
function Link($pred, $succ) { [void]$succ.TaskDependencies.Add($pred, $FS, 0) }
# A (4d) -> B (2d), with C (6d) in parallel so A has a successor and the project a finish.
function Chain($p) {
  $a = Add-Task $p 'A' 4; $b = Add-Task $p 'B' 2; Link $a $b
  [void](Add-Task $p 'C_paralela' 6)
  Write-Output -NoEnumerate $a
}

function Run($name, [scriptblock]$build) {
  if ($Only -and $name -notmatch $Only) { return }
  "== $name"
  try {
    $p = $app.Projects.Add()
    $p.ProjectStart = $Start
    & $build $p
    if ($app.ActiveProject.Name -ne $p.Name) { throw "projeto ativo inesperado: $($app.ActiveProject.Name)" }
    [void]$app.CalculateProject()
    $args19 = @((Join-Path $OutDir "$name-antes.xml"), 0) + @($M) * 7 + @('MSProject.XML') + @($M) * 9
    [void]$app.FileSaveAs.Invoke($args19)
    "  saved $name-antes.xml"
    [void]$app.FileCloseEx(0, $M, $M)   # pjDoNotSave
  } catch { "  ERRO: $($_.Exception.Message) @ $(($_.ScriptStackTrace -split "`n")[0])" }
}

try {
  Run 'B6-P1-50pct' { param($p) $a = Chain $p; $a.PercentComplete = 50 }
  Run 'B6-P2-100pct' { param($p) $a = Chain $p; $a.PercentComplete = 100 }
  Run 'B6-P3-inicio-real-atrasado' { param($p) $a = Chain $p; $a.ActualStart = [datetime]'2026-11-04 09:00' }
  Run 'B6-P4-termino-real-adiantado' {
    param($p)
    $a = Chain $p
    $a.ActualStart = [datetime]'2026-11-02 09:00'; $a.ActualFinish = [datetime]'2026-11-04 18:00'
  }
  Run 'B6-P5-termino-real-atrasado' {
    param($p)
    $a = Chain $p
    $a.ActualStart = [datetime]'2026-11-02 09:00'; $a.ActualFinish = [datetime]'2026-11-10 18:00'
  }
  Run 'B6-P6-duracao-real' { param($p) $a = Chain $p; $a.ActualDuration = 3 * $DAY }
  Run 'B6-P7-trabalho-real' {
    param($p)
    $r = $p.Resources.Add('R1')
    $a = Chain $p; [void]$a.Assignments.Add($a.ID, $r.ID, 1)
    $a.ActualWork = 8 * 60
  }
  Run 'B6-P8-reagendar' {
    param($p)
    $a = Chain $p; $a.PercentComplete = 25
    $p.StatusDate = [datetime]'2026-11-05 18:00'
    [void]$app.UpdateProject($false, [datetime]'2026-11-05 18:00', $RESCHEDULE)
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
