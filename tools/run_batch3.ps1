param([string]$OutDir, [string]$Only = '')
# Batch 3: task types (fixed units / duration / work), effort-driven scheduling and tasks with
# more than one resource. Edit series export MSPDI after every step (`<serie>-<n>.xml`), so each
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

function Run-Leveling([string]$name, [scriptblock]$build, [bool]$individual) {
  if ($Only -and $name -notmatch $Only) { return }
  "== $name (atribuicoesIndividuais=$individual)"
  try {
    $p = New-TestProject
    & $build $p
    Save-Xml $p "$name-antes.xml"
    Assert-Active $p
    # LevelingOptions(Automatic, DelayInSlack, AutoClearLeveling, Order=Standard, LevelEntireProject,
    #                 FromDate, ToDate, PeriodBasis, LevelIndividualAssignments, ...)
    [void]$app.LevelingOptions($false, $false, $true, 1, $true, $M, $M, $M, $individual, $M, $M)
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

$originalIndividual = $app.LevelIndividualAssignments
try {
  "LevelIndividualAssignments (padrao) = $originalIndividual"
  Run-Series 'B3-FU' $FIXED_UNITS $false
  Run-Series 'B3-FU-ED' $FIXED_UNITS $true
  Run-Series 'B3-FD' $FIXED_DURATION $false
  Run-Series 'B3-FD-ED' $FIXED_DURATION $true
  Run-Series 'B3-FW' $FIXED_WORK $true
  Run-HalfTime 'B3-H-FU' $FIXED_UNITS $false
  Run-HalfTime 'B3-H-FU-ED' $FIXED_UNITS $true
  Run-HalfTime 'B3-H-FW' $FIXED_WORK $true
  foreach ($ind in $true, $false) {
    $suffix = if ($ind) { 'indiv' } else { 'tarefa' }
    Run-Leveling "B3-M1-dois-recursos-$suffix" $doisRecursos $ind
    Run-Leveling "B3-M2-unidades-parciais-$suffix" $unidadesParciais $ind
    Run-Leveling "B3-M3-atribuicao-curta-$suffix" $atribuicaoCurta $ind
  }
}
finally {
  try { [void]$app.LevelingOptions($false, $false, $true, 1, $true, $M, $M, $M, $originalIndividual, $M, $M) } catch { }
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}

