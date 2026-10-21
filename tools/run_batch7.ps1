param([string]$OutDir, [string]$Only = '')
# Batch 7: progress on tasks with resources (actual work per assignment, two resources,
# rescheduling with a resource, leveling around a task in progress). CPM only; each case exports one `<case>-antes.xml`.
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

function Get-Assignment($t, [string]$resource) {
  foreach ($a in $t.Assignments) { if ($a.ResourceName -eq $resource) { return Write-Output -NoEnumerate $a } }
  throw "sem atribuicao de $resource em $($t.Name)"
}
function Level($p) {
  [void]$app.LevelingOptions($false, $false, $true, 1, $true, $M, $M, $M, $M, $M, $M)
  [void]$app.LevelNow($true)
}

try {
  Run 'B7-R1-dois-recursos-um-trabalhou' {
    param($p)
    $r1 = $p.Resources.Add('R1'); $r2 = $p.Resources.Add('R2')
    $a = Chain $p
    [void]$a.Assignments.Add($a.ID, $r1.ID, 1); [void]$a.Assignments.Add($a.ID, $r2.ID, 1)
    (Get-Assignment $a 'R1').ActualWork = 16 * 60
  }
  Run 'B7-R2-inicio-real-atrasado-50pct' {
    param($p)
    $r1 = $p.Resources.Add('R1')
    $a = Chain $p; [void]$a.Assignments.Add($a.ID, $r1.ID, 1)
    $a.ActualStart = [datetime]'2026-11-03 09:00'; $a.PercentComplete = 50
  }
  Run 'B7-R3-reagendar-com-recurso' {
    param($p)
    $r1 = $p.Resources.Add('R1')
    $a = Chain $p; [void]$a.Assignments.Add($a.ID, $r1.ID, 1)
    $a.PercentComplete = 25
    $p.StatusDate = [datetime]'2026-11-05 18:00'
    [void]$app.UpdateProject($false, [datetime]'2026-11-05 18:00', $RESCHEDULE)
  }
  Run 'B7-R4-nivelar-com-andamento' {
    param($p)
    $r1 = $p.Resources.Add('R1')
    $t1 = Add-Task $p 'T1_andamento' 3; $t2 = Add-Task $p 'T2_critica' 4
    $t3 = Add-Task $p 'T3_apos_T2' 2; Link $t2 $t3
    foreach ($t in $t1, $t2) { [void]$t.Assignments.Add($t.ID, $r1.ID, 1) }
    $t1.PercentComplete = 30
    Level $p
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
