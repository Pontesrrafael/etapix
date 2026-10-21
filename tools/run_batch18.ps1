param([string]$OutDir, [string]$Only = '')
# Batch 18: baselines (saved dates, duration, work, cost), variances after a slip, and earned
# value (BCWS/BCWP/ACWP) with progress and a status date. CPM only; each case exports one `<case>-antes.xml`.
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
    [void]$app.FileCloseEx(0, $M, $M)
  } catch { "  ERRO: $($_.Exception.Message) @ $(($_.ScriptStackTrace -split "`n")[0])" }
}
function SaveBaseline { [void]$app.BaselineSave($true) }

try {
  Run 'B18-B1-base-e-atraso' {
    param($p)
    $r = $p.Resources.Add('R1'); $r.StandardRate = 100
    $a = Add-Task $p 'A' 3; $b = Add-Task $p 'B' 2; Link $a $b
    [void]$a.Assignments.Add($a.ID, $r.ID, 1)
    $s = Add-Task $p 'C_fixo' 1; $s.FixedCost = 500
    SaveBaseline
    $a.Duration = 5 * $DAY
    $s.FixedCost = 800
  }
  Run 'B18-B2-valor-agregado' {
    param($p)
    $r = $p.Resources.Add('R1'); $r.StandardRate = 100
    $a = Add-Task $p 'A' 4; $b = Add-Task $p 'B' 2; Link $a $b
    [void]$a.Assignments.Add($a.ID, $r.ID, 1); [void]$b.Assignments.Add($b.ID, $r.ID, 1)
    SaveBaseline
    $p.StatusDate = [datetime]'2026-11-04 18:00'
    $a.PercentComplete = 50
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
