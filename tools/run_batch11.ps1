param([string]$OutDir, [string]$Only = '')
# Batch 11: critical slack limit, multiple critical paths and leveling period bases. CPM only; each case exports one `<case>-antes.xml`.
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
    if ($script:Level) { [void]$app.LevelingOptions($false, $false, $true, 1, $true, $M, $M, $script:Basis, $M, $M, $M); [void]$app.LevelNow($true); $script:Level = $false }
    if ($app.ActiveProject.Name -ne $p.Name) { throw "projeto ativo inesperado: $($app.ActiveProject.Name)" }
    [void]$app.CalculateProject()
    $args19 = @((Join-Path $OutDir "$name-antes.xml"), 0) + @($M) * 7 + @('MSProject.XML') + @($M) * 9
    [void]$app.FileSaveAs.Invoke($args19)
    "  saved $name-antes.xml"
    [void]$app.FileCloseEx(0, $M, $M)
  } catch { "  ERRO: $($_.Exception.Message) @ $(($_.ScriptStackTrace -split "`n")[0])" }
}
# OptionsCalculation(Automatic, AutoTrack, SpreadPercentToStatusDate, SpreadCostsToStatusDate,
#   AutoCalcCosts, FixedCostAccrual, CalcMultipleCriticalPaths, CriticalSlack, SetDefaults=false)
function CalcOptions([bool]$multiple, $slackMinutes) {
  [void]$app.OptionsCalculation($M, $M, $M, $M, $M, $M, $multiple, $slackMinutes, $false, $M, $M, $M, $M, $M, $M, $M)
}
function Chains($p) {
  $a = Add-Task $p 'A' 3; $b = Add-Task $p 'B' 2; Link $a $b
  $c = Add-Task $p 'C' 2; $d = Add-Task $p 'D' 1; Link $c $d
  [void](Add-Task $p 'E_4d' 4)
}
$script:Level = $false; $script:Basis = 2

try {
  Run 'B11-K1-limite-folga-1d' { param($p) Chains $p; CalcOptions $false (1 * $DAY) }
  Run 'B11-K2-multiplos-caminhos' { param($p) Chains $p; CalcOptions $true 0 }
  foreach ($b in @(@{n='semana'; v=3}, @{n='dia'; v=2}, @{n='hora'; v=1})) {
    Run "B11-L1-base-$($b.n)" {
      param($p)
      $r = $p.Resources.Add('R1')
      $t1 = $p.Tasks.Add('T1_4h'); $t1.Manual = $false; $t1.Duration = 240
      $t2 = $p.Tasks.Add('T2_4h'); $t2.Manual = $false; $t2.Duration = 240
      $t3 = Add-Task $p 'T3_2d' 2; $t4 = Add-Task $p 'T4_2d' 2
      foreach ($t in $t1, $t2, $t3, $t4) { [void]$t.Assignments.Add($t.ID, $r.ID, 1) }
      $script:Level = $true; $script:Basis = $b.v
    }
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
