param([string]$OutDir, [string]$Only = '')
# Batch 15: leveling individual assignments vs whole tasks, leveling a date range only, and
# splitting tasks whose assignments are uneven. Edit series export MSPDI after every step (`<serie>-<n>.xml`), so each
# recalculation can be replayed; leveling cases export `-antes`/`-depois` like the earlier batches.
$ErrorActionPreference = 'Stop'
$M = [System.Reflection.Missing]::Value
$DAY = 480
$Start = [datetime]'2026-11-02 08:00'   # Monday
# PjTaskFixedType
$FIXED_UNITS = 0; $FIXED_DURATION = 1; $FIXED_WORK = 2

$app = New-Object -ComObject MSProject.Application
$app.Visible = $false
$app.DisplayAlerts = $false
"Project version: $($app.Version) build $($app.Build)"

function New-TestProject {
  $p = $app.Projects.Add()
  $p.ProjectStart = $Start
  Write-Output -NoEnumerate $p
}
function Add-Task($p, [string]$name, [int]$days, [int]$type = $FIXED_UNITS, [bool]$effort = $false) {
  $t = $p.Tasks.Add($name)
  $t.Manual = $false
  $t.Type = $type
  # Fixed-work tasks are always effort-driven and reject writes to the flag.
  if ($type -ne $FIXED_WORK) { $t.EffortDriven = $effort }
  $t.Duration = $days * $DAY
  Write-Output -NoEnumerate $t
}
function Get-Assignment($t, [string]$resource) {
  foreach ($a in $t.Assignments) { if ($a.ResourceName -eq $resource) { return Write-Output -NoEnumerate $a } }
  throw "sem atribuicao de $resource em $($t.Name)"
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
function Close-Project($p) {
  Assert-Active $p
  [void]$app.FileCloseEx(0, $M, $M)   # pjDoNotSave
}

# One task edited step by step; every step is exported.
function Run-Series([string]$name, [int]$type, [bool]$effort) {
  if ($Only -and $name -notmatch $Only) { return }
  "== $name"
  try {
    $p = New-TestProject
    $r1 = $p.Resources.Add('R1'); $r2 = $p.Resources.Add('R2')
    $t = Add-Task $p 'X' 4 $type $effort
    [void]$t.Assignments.Add($t.ID, $r1.ID, 1)
    Save-Xml $p "$name-0-base-4d-R1.xml"
    [void]$t.Assignments.Add($t.ID, $r2.ID, 1)
    Save-Xml $p "$name-1-add-R2.xml"
    $t.Duration = 6 * $DAY
    Save-Xml $p "$name-2-dur-6d.xml"
    (Get-Assignment $t 'R1').Units = 0.5
    Save-Xml $p "$name-3-R1-50pct.xml"
    $t.Work = 80 * 60
    Save-Xml $p "$name-4-work-80h.xml"
    (Get-Assignment $t 'R2').Work = 40 * 60
    Save-Xml $p "$name-5-R2-work-40h.xml"
    Close-Project $p
  } catch { "  ERRO: $($_.Exception.Message) @ $(($_.ScriptStackTrace -split "`n")[0])" }
}

# Effort-driven: adding a half-time second resource to a 3d full-time task.
function Run-HalfTime([string]$name, [int]$type, [bool]$effort) {
  if ($Only -and $name -notmatch $Only) { return }
  "== $name"
  try {
    $p = New-TestProject
    $r1 = $p.Resources.Add('R1'); $r2 = $p.Resources.Add('R2')
    $t = Add-Task $p 'X' 3 $type $effort
    [void]$t.Assignments.Add($t.ID, $r1.ID, 1)
    Save-Xml $p "$name-0-base-3d-R1.xml"
    [void]$t.Assignments.Add($t.ID, $r2.ID, 0.5)
    Save-Xml $p "$name-1-add-R2-50pct.xml"
    (Get-Assignment $t 'R1').Delete()
    Save-Xml $p "$name-2-del-R1.xml"
    Close-Project $p
  } catch { "  ERRO: $($_.Exception.Message) @ $(($_.ScriptStackTrace -split "`n")[0])" }
}

function Run-Leveling([string]$name, [scriptblock]$build, [bool]$individual, $from = $null, $to = $null) {
  if ($Only -and $name -notmatch $Only) { return }
  "== $name (atribuicoesIndividuais=$individual)"
  try {
    $p = New-TestProject
    & $build $p
    Save-Xml $p "$name-antes.xml"
    Assert-Active $p
    # LevelingOptions(Automatic, DelayInSlack, AutoClearLeveling, Order=Standard, LevelEntireProject,
    #                 FromDate, ToDate, PeriodBasis, LevelIndividualAssignments, ...)
    if ($from) {
      [void]$app.LevelingOptions($false, $false, $true, 1, $false, [datetime]$from, [datetime]$to, $M, $individual, $M, $M)
    } else {
      [void]$app.LevelingOptions($false, $false, $true, 1, $true, $M, $M, $M, $individual, $M, $M)
    }
    [void]$app.LevelNow($true)
    Save-Xml $p "$name-depois.xml"
    Close-Project $p
  } catch { "  ERRO: $($_.Exception.Message) @ $(($_.ScriptStackTrace -split "`n")[0])" }
}

# M1: a two-resource task competes with one single-resource task on each resource.
$doisRecursos = {
  param($p)
  $r1 = $p.Resources.Add('R1'); $r2 = $p.Resources.Add('R2')
  $t1 = Add-Task $p 'T1_R1+R2' 3; $t2 = Add-Task $p 'T2_R2' 2; $t3 = Add-Task $p 'T3_R1' 2
  [void]$t1.Assignments.Add($t1.ID, $r1.ID, 1); [void]$t1.Assignments.Add($t1.ID, $r2.ID, 1)
  [void]$t2.Assignments.Add($t2.ID, $r2.ID, 1)
  [void]$t3.Assignments.Add($t3.ID, $r1.ID, 1)
}
# M2: partial units add up on R2 (50% + 50% fits; the 100% task does not).
$unidadesParciais = {
  param($p)
  $r1 = $p.Resources.Add('R1'); $r2 = $p.Resources.Add('R2')
  $t1 = Add-Task $p 'T1_R1+R2meio' 3; $t2 = Add-Task $p 'T2_R2meio' 2; $t3 = Add-Task $p 'T3_R2' 2
  [void]$t1.Assignments.Add($t1.ID, $r1.ID, 1); [void]$t1.Assignments.Add($t1.ID, $r2.ID, 0.5)
  [void]$t2.Assignments.Add($t2.ID, $r2.ID, 0.5)
  [void]$t3.Assignments.Add($t3.ID, $r2.ID, 1)
}
# M3: R2 works only the first 2 of T1's 4 days; T2 needs R2 for 2 days.
$atribuicaoCurta = {
  param($p)
  $r1 = $p.Resources.Add('R1'); $r2 = $p.Resources.Add('R2')
  $t1 = Add-Task $p 'T1_R1_4d+R2_2d' 4; $t2 = Add-Task $p 'T2_R2' 2
  [void]$t1.Assignments.Add($t1.ID, $r1.ID, 1); [void]$t1.Assignments.Add($t1.ID, $r2.ID, 1)
  (Get-Assignment $t1 'R2').Work = 2 * $DAY
  [void]$t2.Assignments.Add($t2.ID, $r2.ID, 1)
}

# I: T1 uses R1 and R2 for 4 days; T2 needs R2 for 2 days and is critical (T3 follows it).
$individualCase = {
  param($p)
  $r1 = $p.Resources.Add('R1'); $r2 = $p.Resources.Add('R2')
  $t1 = Add-Task $p 'T1_R1+R2' 4; $t2 = Add-Task $p 'T2_R2' 2; $t3 = Add-Task $p 'T3_apos_T2' 6
  [void]$t3.TaskDependencies.Add($t2, 1, 0)
  $t1.LevelingCanSplit = $false
  [void]$t1.Assignments.Add($t1.ID, $r1.ID, 1); [void]$t1.Assignments.Add($t1.ID, $r2.ID, 1)
  [void]$t2.Assignments.Add($t2.ID, $r2.ID, 1)
}
# D: two separate conflicts on R1, week 1 and week 3; only week 3 is in the leveling range.
$rangeCase = {
  param($p)
  $r = $p.Resources.Add('R1')
  $a1 = Add-Task $p 'A1' 2; $a2 = Add-Task $p 'A2' 2
  $b1 = Add-Task $p 'B1_sem3' 2; $b2 = Add-Task $p 'B2_sem3' 2
  foreach ($t in $b1, $b2) { $t.ConstraintType = 4; $t.ConstraintDate = [datetime]'2026-11-16 09:00' }
  foreach ($t in $a1, $a2, $b1, $b2) { [void]$t.Assignments.Add($t.ID, $r.ID, 1) }
}
# S: uneven assignments (R2 works only the first 2 of T1's 4 days) with splitting allowed.
$unevenSplit = {
  param($p)
  $r1 = $p.Resources.Add('R1'); $r2 = $p.Resources.Add('R2')
  $t0 = Add-Task $p 'T0_R2_d2' 1; $t0.ConstraintType = 4; $t0.ConstraintDate = [datetime]'2026-11-03 09:00'; $t0.Priority = 1000
  $t1 = Add-Task $p 'T1_R1_4d+R2_2d' 4
  [void]$t1.Assignments.Add($t1.ID, $r1.ID, 1); [void]$t1.Assignments.Add($t1.ID, $r2.ID, 1)
  (Get-Assignment $t1 'R2').Work = 2 * $DAY
  [void]$t0.Assignments.Add($t0.ID, $r2.ID, 1)
}

$originalIndividual = $app.LevelIndividualAssignments
try {
  Run-Leveling 'B15-I-indiv' $individualCase $true
  Run-Leveling 'B15-I-tarefa' $individualCase $false
  Run-Leveling 'B15-D-intervalo' $rangeCase $true '2026-11-13' '2026-11-20'
  Run-Leveling 'B15-D-tudo' $rangeCase $true
  Run-Leveling 'B15-S-divisao-desigual' $unevenSplit $true
}
finally {
  try { [void]$app.LevelingOptions($false, $false, $true, 1, $true, $M, $M, $M, $originalIndividual, $M, $M) } catch { }
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
