param([string]$OutDir)
# Builds small test projects through the documented MSProject COM object model, exports each as
# MSPDI XML, then (for leveling scenarios) runs LevelNow and exports again.
$ErrorActionPreference = 'Stop'
$M = [System.Reflection.Missing]::Value
$DAY = 480                       # minutes per working day in the Standard calendar
$Start = [datetime]'2026-11-02 08:00'   # Monday

# PjTaskLinkType
$FF = 0; $FS = 1; $SF = 2; $SS = 3
# PjConstraint
$SNET = 4

$app = New-Object -ComObject MSProject.Application
$app.Visible = $false
$app.DisplayAlerts = $false
"Project version: $($app.Version) build $($app.Build)"

function New-TestProject([string]$name) {
  $p = $app.Projects.Add()
  $p.ProjectStart = $Start
  Write-Output -NoEnumerate $p   # COM Project is enumerable; keep the object itself
}
function Add-Task($p, [string]$name, [int]$days) {
  $t = $p.Tasks.Add($name)
  $t.Manual = $false
  $t.Duration = $days * $DAY
  Write-Output -NoEnumerate $t
}
function Link($pred, $succ, [int]$type, [int]$lagMinutes = 0) {
  [void]$succ.TaskDependencies.Add($pred, $type, $lagMinutes)
}
function Save-Xml($p, [string]$file) {
  if ($app.ActiveProject.Name -ne $p.Name) { throw "projeto ativo inesperado: $($app.ActiveProject.Name)" }
  [void]$app.CalculateProject()
  $path = Join-Path $OutDir $file
  # The typed binding wants all 19 arguments; Format 0 (pjMPP) is overridden by FormatID.
  $args19 = @($path, 0) + @($M) * 7 + @('MSProject.XML') + @($M) * 9
  [void]$app.FileSaveAs.Invoke($args19)
  "  saved $file"
}
function Close-Project($p) {
  if ($app.ActiveProject.Name -ne $p.Name) { throw "projeto ativo inesperado: $($app.ActiveProject.Name)" }
  [void]$app.FileCloseEx(0, $M, $M)   # pjDoNotSave
}
function Run($name, [scriptblock]$build, [bool]$level) {
  "== $name"
  try {
    $p = New-TestProject $name
    & $build $p
    Save-Xml $p "$name-antes.xml"
    if ($level) {
      if ($app.ActiveProject.Name -ne $p.Name) { throw "projeto ativo inesperado: $($app.ActiveProject.Name)" }
      [void]$app.LevelNow($true)
      Save-Xml $p "$name-depois.xml"
    }
    Close-Project $p
  } catch { "  ERRO: $($_.Exception.Message) @ $(($_.ScriptStackTrace -split "`n")[0])" }
}

try {
  # 1. Critical path: two parallel branches of different length merging into D.
  Run 'S1-caminho-critico' {
    param($p)
    $a = Add-Task $p 'A' 5; $b = Add-Task $p 'B' 3; $c = Add-Task $p 'C' 1; $d = Add-Task $p 'D' 2
    Link $a $b $FS; Link $a $c $FS; Link $b $d $FS; Link $c $d $FS
  } $false

  # 2. Every link type, with lag and lead.
  Run 'S2-tipos-dependencia' {
    param($p)
    $a = Add-Task $p 'A' 5
    $b = Add-Task $p 'B_SS+2d' 4; Link $a $b $SS (2 * $DAY)
    $c = Add-Task $p 'C_FF+1d' 2; Link $a $c $FF (1 * $DAY)
    $d = Add-Task $p 'D_FS-1d' 3; Link $a $d $FS (-1 * $DAY)
    $e = Add-Task $p 'E_SF' 2;    Link $a $e $SF 0
    $f = Add-Task $p 'F_fim' 1;   Link $b $f $FS; Link $c $f $FS; Link $d $f $FS; Link $e $f $FS
  } $false

  # 3. One resource (100%) overallocated by two parallel tasks.
  Run 'S3-nivelamento-basico' {
    param($p)
    $r = $p.Resources.Add('R1'); $r.MaxUnits = 1
    $t1 = Add-Task $p 'T1' 4; $t2 = Add-Task $p 'T2' 3; $t3 = Add-Task $p 'T3' 2
    Link $t1 $t3 $FS
    foreach ($t in $t1, $t2, $t3) { [void]$t.Assignments.Add($t.ID, $r.ID, 1) }
  } $true

  # 4. Same as 3, but T2 gets a higher priority than T1.
  Run 'S4-nivelamento-prioridade' {
    param($p)
    $r = $p.Resources.Add('R1'); $r.MaxUnits = 1
    $t1 = Add-Task $p 'T1' 4; $t2 = Add-Task $p 'T2' 3; $t3 = Add-Task $p 'T3' 2
    Link $t1 $t3 $FS
    $t1.Priority = 500; $t2.Priority = 900
    foreach ($t in $t1, $t2, $t3) { [void]$t.Assignments.Add($t.ID, $r.ID, 1) }
  } $true

  # 5. Leveling that may split: long low-priority task vs. short high-priority task pinned to day 3.
  Run 'S5-nivelamento-divisao' {
    param($p)
    $r = $p.Resources.Add('R1'); $r.MaxUnits = 1
    $long = Add-Task $p 'Longa' 6; $long.Priority = 100; $long.LevelingCanSplit = $true
    $short = Add-Task $p 'Curta_SNET_d3' 2; $short.Priority = 1000
    $short.ConstraintType = $SNET; $short.ConstraintDate = $Start.AddDays(2)
    foreach ($t in $long, $short) { [void]$t.Assignments.Add($t.ID, $r.ID, 1) }
  } $true

  # 6. Same as 5 with splitting disabled on the long task.
  Run 'S6-nivelamento-sem-divisao' {
    param($p)
    $r = $p.Resources.Add('R1'); $r.MaxUnits = 1
    $long = Add-Task $p 'Longa' 6; $long.Priority = 100; $long.LevelingCanSplit = $false
    $short = Add-Task $p 'Curta_SNET_d3' 2; $short.Priority = 1000
    $short.ConstraintType = $SNET; $short.ConstraintDate = $Start.AddDays(2)
    foreach ($t in $long, $short) { [void]$t.Assignments.Add($t.ID, $r.ID, 1) }
  } $true

  # 7. Non-working day in the project calendar in the middle of a chain.
  Run 'S7-calendario' {
    param($p)
    $p.Calendar.Period([datetime]'2026-11-04', [datetime]'2026-11-04').Working = $false   # Wednesday off
    $a = Add-Task $p 'A' 3; $b = Add-Task $p 'B' 2; Link $a $b $FS
  } $false

  # 8. Resource at 50% max units: does leveling stretch or delay?
  Run 'S8-unidades-50' {
    param($p)
    $r = $p.Resources.Add('R_meio'); $r.MaxUnits = 0.5
    $t1 = Add-Task $p 'T1' 2; $t2 = Add-Task $p 'T2' 2
    [void]$t1.Assignments.Add($t1.ID, $r.ID, 0.5)
    [void]$t2.Assignments.Add($t2.ID, $r.ID, 0.5)
  } $true
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}


