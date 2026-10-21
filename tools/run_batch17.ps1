param([string]$OutDir, [string]$Only = '')
# Batch 17: costs. Hourly and overtime rates, cost per use, fixed cost and its accrual, material
# resources, rate changes over time and cost rollup to summaries. CPM only; each case exports one `<case>-antes.xml`.
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
function Res($p, [string]$name, $rate, $ot = 0, $perUse = 0) {
  $r = $p.Resources.Add($name)
  $r.StandardRate = $rate; $r.OvertimeRate = $ot; $r.CostPerUse = $perUse
  Write-Output -NoEnumerate $r
}

try {
  Run 'B17-C1-taxa-e-custo-por-uso' {
    param($p)
    $r = Res $p 'R1' 100 0 50
    $a = Add-Task $p 'A' 4; [void]$a.Assignments.Add($a.ID, $r.ID, 1)
  }
  Run 'B17-C2-dois-recursos-meio' {
    param($p)
    $r1 = Res $p 'R1' 100; $r2 = Res $p 'R2' 60
    $a = Add-Task $p 'A' 3; [void]$a.Assignments.Add($a.ID, $r1.ID, 1); [void]$a.Assignments.Add($a.ID, $r2.ID, 0.5)
  }
  Run 'B17-C3-custo-fixo' {
    param($p)
    $a = Add-Task $p 'A_inicio' 4; $a.FixedCost = 1000; $a.FixedCostAccrual = 1
    $b = Add-Task $p 'B_rateado' 4; $b.FixedCost = 1000; $b.FixedCostAccrual = 2
    $c = Add-Task $p 'C_fim' 4; $c.FixedCost = 1000; $c.FixedCostAccrual = 3
  }
  Run 'B17-C4-material' {
    param($p)
    $m = $p.Resources.Add('Cimento'); $m.Type = 1; $m.StandardRate = 20; $m.MaterialLabel = 'saco'
    $a = Add-Task $p 'A' 2; [void]$a.Assignments.Add($a.ID, $m.ID, 1); foreach ($as in $a.Assignments) { $as.Units = 10 }
  }
  Run 'B17-C5-hora-extra' {
    param($p)
    $r = Res $p 'R1' 100 150
    $a = Add-Task $p 'A' 4; [void]$a.Assignments.Add($a.ID, $r.ID, 1)
    foreach ($as in $a.Assignments) { $as.OvertimeWork = 8 * 60 }
  }
  Run 'B17-C6-mudanca-de-taxa' {
    param($p)
    $r = Res $p 'R1' 100
    [void]$r.CostRateTables.Item(1).PayRates.Add([datetime]'2026-11-04', 200)
    $a = Add-Task $p 'A' 4; [void]$a.Assignments.Add($a.ID, $r.ID, 1)
  }
  Run 'B17-C7-resumo' {
    param($p)
    $r = Res $p 'R1' 100
    $s = Add-Task $p 'S' 1
    $a = Add-Task $p 'A' 2; $a.OutlineIndent(); [void]$a.Assignments.Add($a.ID, $r.ID, 1)
    $b = Add-Task $p 'B' 1; $b.FixedCost = 300; Link $a $b
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
