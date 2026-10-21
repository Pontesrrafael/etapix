param([string]$OutDir)
# Second batch: same overallocation setups leveled under each leveling order
# (ID, Standard, Priority/Standard) and with "level only within available slack".
$ErrorActionPreference = 'Stop'
$M = [System.Reflection.Missing]::Value
$DAY = 480
$Start = [datetime]'2026-11-02 08:00'   # Monday
$FS = 1; $SNET = 4
# PjLevelOrder
$ORDERS = [ordered]@{ 'ID' = 0; 'Standard' = 1; 'Prioridade' = 2 }

$app = New-Object -ComObject MSProject.Application
$app.Visible = $false
$app.DisplayAlerts = $false
"Project version: $($app.Version) build $($app.Build)"

function New-TestProject {
  $p = $app.Projects.Add()
  $p.ProjectStart = $Start
  Write-Output -NoEnumerate $p
}
function Add-Task($p, [string]$name, [int]$days) {
  $t = $p.Tasks.Add($name)
  $t.Manual = $false
  $t.Duration = $days * $DAY
  Write-Output -NoEnumerate $t
}
function Assign($p, $resource, [object[]]$tasks, [double]$units = 1) {
  foreach ($t in $tasks) { [void]$t.Assignments.Add($t.ID, $resource.ID, $units) }
}
function Assert-Active($p) {
  if ($app.ActiveProject.Name -ne $p.Name) { throw "projeto ativo inesperado: $($app.ActiveProject.Name)" }
}
function Save-Xml($p, [string]$file) {
  Assert-Active $p
  [void]$app.CalculateProject()
  $args19 = @((Join-Path $OutDir $file), 0) + @($M) * 7 + @('MSProject.XML') + @($M) * 9
  [void]$app.FileSaveAs.Invoke($args19)
  "  saved $file"
}
function Run($name, [scriptblock]$build, [int]$order, [bool]$withinSlack) {
  "== $name (ordem=$order, dentroDaFolga=$withinSlack)"
  try {
    $p = New-TestProject
    & $build $p
    Save-Xml $p "$name-antes.xml"
    Assert-Active $p
    # LevelingOptions(Automatic, DelayInSlack, AutoClearLeveling, Order, LevelEntireProject, ...)
    [void]$app.LevelingOptions($false, $withinSlack, $true, $order, $true, $M, $M, $M, $M, $M, $M)
    [void]$app.LevelNow($true)
    Save-Xml $p "$name-depois.xml"
    Assert-Active $p
    [void]$app.FileCloseEx(0, $M, $M)   # pjDoNotSave
  } catch { "  ERRO: $($_.Exception.Message) @ $(($_.ScriptStackTrace -split "`n")[0])" }
}

# A: T1 (prio 500) -> T3 is the critical chain; T2 (prio 900) has 3d slack. One resource.
$prioridadeVsFolga = {
  param($p)
  $r = $p.Resources.Add('R1'); $r.MaxUnits = 1
  $t1 = Add-Task $p 'T1_p500' 4; $t2 = Add-Task $p 'T2_p900' 3; $t3 = Add-Task $p 'T3_p500' 2
  [void]$t3.TaskDependencies.Add($t1, $FS, 0)
  $t1.Priority = 500; $t2.Priority = 900; $t3.Priority = 500
  Assign $p $r @($t1, $t2, $t3)
}
# B: long low-priority task vs short high-priority (900, still levelable) task pinned to day 3; no splits.
$longaVsCurta = {
  param($p)
  $r = $p.Resources.Add('R1'); $r.MaxUnits = 1
  $long = Add-Task $p 'Longa_p100' 6; $long.Priority = 100; $long.LevelingCanSplit = $false
  $short = Add-Task $p 'Curta_p900_SNETd3' 2; $short.Priority = 900; $short.LevelingCanSplit = $false
  $short.ConstraintType = $SNET; $short.ConstraintDate = $Start.AddDays(2)
  Assign $p $r @($long, $short)
}
# C1: the S3 conflict; T2 has only 3d slack but needs more delay to clear the overallocation.
$folgaCurta = {
  param($p)
  $r = $p.Resources.Add('R1'); $r.MaxUnits = 1
  $t1 = Add-Task $p 'T1' 4; $t2 = Add-Task $p 'T2' 3; $t3 = Add-Task $p 'T3' 2
  [void]$t3.TaskDependencies.Add($t1, $FS, 0)
  Assign $p $r @($t1, $t2, $t3)
}
# C2: same conflict, plus an unassigned 15d task that drives the finish, so T2 has plenty of slack.
$folgaLonga = {
  param($p)
  $r = $p.Resources.Add('R1'); $r.MaxUnits = 1
  $t1 = Add-Task $p 'T1' 4; $t2 = Add-Task $p 'T2' 3; $t3 = Add-Task $p 'T3' 2
  [void]$t3.TaskDependencies.Add($t1, $FS, 0)
  [void](Add-Task $p 'Marco_15d_sem_recurso' 15)
  Assign $p $r @($t1, $t2, $t3)
}

try {
  foreach ($o in $ORDERS.GetEnumerator()) {
    Run "B2-A-prioridade-vs-folga-$($o.Key)" $prioridadeVsFolga $o.Value $false
    Run "B2-B-longa-vs-curta-$($o.Key)" $longaVsCurta $o.Value $false
  }
  Run 'B2-C1-folga-curta-dentroDaFolga' $folgaCurta 1 $true
  Run 'B2-C1-folga-curta-normal' $folgaCurta 1 $false
  Run 'B2-C2-folga-longa-dentroDaFolga' $folgaLonga 1 $true
  Run 'B2-C2-folga-longa-normal' $folgaLonga 1 $false
}
finally {
  # Put leveling back to Project's defaults: manual, Standard order, not limited to slack.
  try { [void]$app.LevelingOptions($false, $false, $true, 1, $true, $M, $M, $M, $M, $M, $M) } catch { }
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
