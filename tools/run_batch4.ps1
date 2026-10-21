param([string]$OutDir, [string]$Only = '')
# Batch 4: every constraint type (including conflicts with predecessors), deadlines and summary
# tasks. CPM only; each case exports one `<case>-antes.xml`.
$ErrorActionPreference = 'Stop'
$M = [System.Reflection.Missing]::Value
$DAY = 480
$Start = [datetime]'2026-11-02 08:00'   # Monday
$FS = 1
# PjConstraint
$ALAP = 1; $MSO = 2; $MFO = 3; $SNET = 4; $SNLT = 5; $FNET = 6; $FNLT = 7

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
function Constrain($t, [int]$type, [string]$date) {
  $t.ConstraintType = $type
  $t.ConstraintDate = [datetime]$date
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

# A (3d) -> B (2d); the constraint goes on B.
function Chain($p) {
  $a = Add-Task $p 'A' 3; $b = Add-Task $p 'B' 2; Link $a $b
  Write-Output -NoEnumerate $b
}

try {
  Run 'B4-K1-ALAP' {
    param($p)
    $b = Chain $p
    $c = Add-Task $p 'C_ALAP' 2; $c.ConstraintType = $ALAP
  }
  Run 'B4-K2-MSO-folga' { param($p) $b = Chain $p; Constrain $b $MSO '2026-11-09 09:00' }
  Run 'B4-K3-MSO-conflito' { param($p) $b = Chain $p; Constrain $b $MSO '2026-11-03 09:00' }
  Run 'B4-K4-MFO' { param($p) $b = Chain $p; Constrain $b $MFO '2026-11-11 18:00' }
  Run 'B4-K5-SNLT-conflito' { param($p) $b = Chain $p; Constrain $b $SNLT '2026-11-03 09:00' }
  Run 'B4-K5b-SNLT-folga' { param($p) $b = Chain $p; Constrain $b $SNLT '2026-11-10 09:00' }
  Run 'B4-K6-FNET' { param($p) $b = Chain $p; Constrain $b $FNET '2026-11-12 18:00' }
  Run 'B4-K7-FNLT-conflito' { param($p) $b = Chain $p; Constrain $b $FNLT '2026-11-04 18:00' }
  Run 'B4-K7b-FNLT-folga' { param($p) $b = Chain $p; Constrain $b $FNLT '2026-11-13 18:00' }
  Run 'B4-K8-deadline' {
    param($p)
    $b = Chain $p; $b.Deadline = [datetime]'2026-11-05 18:00'   # missed: B finishes 11-06
    $c = Add-Task $p 'C_4d' 4; $c.Deadline = [datetime]'2026-11-12 18:00'   # met
    $d = Add-Task $p 'D_1d' 1; $d.Deadline = [datetime]'2026-11-03 18:00'   # met, tighter than finish
  }
  Run 'B4-K9-resumo' {
    param($p)
    $s1 = Add-Task $p 'S1' 1
    $a = Add-Task $p 'A' 3; $a.OutlineIndent()
    $b = Add-Task $p 'B' 2; Link $a $b
    $s2 = Add-Task $p 'S2' 1; $s2.OutlineOutdent()
    $c = Add-Task $p 'C' 2; $c.OutlineIndent()
    $d = Add-Task $p 'D' 1; Link $c $d
    $e = Add-Task $p 'E_solta' 1; $e.OutlineOutdent()
    Link $s1 $s2
  }
  Run 'B4-K10-resumo-restricao' {
    param($p)
    $s1 = Add-Task $p 'S1' 1
    $a = Add-Task $p 'A' 2; $a.OutlineIndent()
    $b = Add-Task $p 'B_SNET' 1; Constrain $b $SNET '2026-11-05 09:00'
    $c = Add-Task $p 'C_apos_S1' 2; $c.OutlineOutdent()
    Link $s1 $c
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
