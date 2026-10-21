param([string]$OutDir, [string]$Only = '')
# Batch 9: milestones, recurring calendar exceptions and tasks whose resources are on
# different calendars. CPM only; each case exports one `<case>-antes.xml`.
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
$DAILY = 1; $WEEKLY = 6

try {
  Run 'B9-Z1-marcos' {
    param($p)
    $a = Add-Task $p 'A' 3
    $m = Add-Task $p 'M_marco' 0; Link $a $m
    $b = Add-Task $p 'B_apos_marco' 2; Link $m $b
    $m2 = Add-Task $p 'M2_inicio' 0
    $c = Add-Task $p 'C_apos_M2' 1; Link $m2 $c
    $m3 = Add-Task $p 'M3_fim_projeto' 0; Link $b $m3; Link $c $m3
  }
  Run 'B9-Z2-marco-restricao' {
    param($p)
    $a = Add-Task $p 'A' 2
    $m = Add-Task $p 'M_SNET' 0; $m.ConstraintType = 4; $m.ConstraintDate = [datetime]'2026-11-06 09:00'
    $b = Add-Task $p 'B' 1; Link $m $b; Link $a $b
  }
  Run 'B9-E1-feriado-recorrente-semanal' {
    param($p)
    # Every Wednesday off for 3 occurrences (weekly exception).
    $e = $p.Calendar.Exceptions.Add($WEEKLY, [datetime]'2026-11-04', [datetime]'2026-11-18')
    $e.DaysOfWeek = 8   # pjWednesday bit (Sun=1, Mon=2, Tue=4, Wed=8)
    $a = Add-Task $p 'A' 8
    $b = Add-Task $p 'B' 2; Link $a $b
  }
  Run 'B9-E2-excecao-varios-dias' {
    param($p)
    [void]$p.Calendar.Exceptions.Add($DAILY, [datetime]'2026-11-04', [datetime]'2026-11-06')
    $a = Add-Task $p 'A' 4
  }
  Run 'B9-C1-recursos-calendarios-diferentes' {
    param($p)
    $r1 = $p.Resources.Add('R1')
    $r2 = $p.Resources.Add('R2'); [void]$r2.Calendar.Exceptions.Add($DAILY, [datetime]'2026-11-03', [datetime]'2026-11-03')
    $t = Add-Task $p 'T_R1_R2' 3
    [void]$t.Assignments.Add($t.ID, $r1.ID, 1); [void]$t.Assignments.Add($t.ID, $r2.ID, 1)
    $v = Add-Task $p 'V_apos_T' 1; Link $t $v
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
