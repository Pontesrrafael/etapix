param([string]$OutDir, [string]$Only = '')
# Batch 10: as-late-as-possible tasks with successors, constraints on summary tasks and
# finish-to-finish / start-to-start links into and out of summaries. CPM only; each case exports one `<case>-antes.xml`.
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

$SS = 3; $FF = 0
function LinkT($pred, $succ, $type) { [void]$succ.TaskDependencies.Add($pred, $type, 0) }

try {
  Run 'B10-A1-ALAP-com-sucessora' {
    param($p)
    $a = Add-Task $p 'A_ALAP' 2; $a.ConstraintType = 1
    $b = Add-Task $p 'B_apos_A' 1; Link $a $b
    [void](Add-Task $p 'C_paralela' 6)
  }
  Run 'B10-A2-ALAP-cadeia' {
    param($p)
    $a = Add-Task $p 'A' 1
    $b = Add-Task $p 'B_ALAP' 2; $b.ConstraintType = 1; Link $a $b
    $c = Add-Task $p 'C' 1; Link $b $c
    [void](Add-Task $p 'D_paralela' 7)
  }
  Run 'B10-S1-resumo-SNET' {
    param($p)
    $s = Add-Task $p 'S' 1
    $a = Add-Task $p 'A' 2; $a.OutlineIndent()
    $b = Add-Task $p 'B' 1; Link $a $b
    $s.ConstraintType = 4; $s.ConstraintDate = [datetime]'2026-11-05 09:00'
    $c = Add-Task $p 'C_solta' 1; $c.OutlineOutdent()
  }
  Run 'B10-L1-FF-para-resumo' {
    param($p)
    $x = Add-Task $p 'X' 5
    $s = Add-Task $p 'S' 1
    $a = Add-Task $p 'A' 2; $a.OutlineIndent()
    $b = Add-Task $p 'B' 1
    $c = Add-Task $p 'C_solta' 1; $c.OutlineOutdent()
    LinkT $x $s $FF
  }
  Run 'B10-L2-SS-do-resumo' {
    param($p)
    $s = Add-Task $p 'S' 1
    $a = Add-Task $p 'A' 2; $a.OutlineIndent()
    $b = Add-Task $p 'B' 3; $b.ConstraintType = 4; $b.ConstraintDate = [datetime]'2026-11-04 09:00'
    $y = Add-Task $p 'Y' 1; $y.OutlineOutdent()
    LinkT $s $y $SS
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
