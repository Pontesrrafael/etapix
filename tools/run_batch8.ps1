param([string]$OutDir, [string]$Only = '')
# Batch 8: manually scheduled tasks (fixed dates that ignore links, and their effect on
# auto-scheduled successors and summaries). CPM only; each case exports one `<case>-antes.xml`.
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
function Manual($p, [string]$name, [string]$start, [int]$days) {
  $t = $p.Tasks.Add($name)
  $t.Manual = $true
  $t.Start = [datetime]$start
  $t.Duration = $days * $DAY
  Write-Output -NoEnumerate $t
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
    [void]$app.FileCloseEx(0, $M, $M)
  } catch { "  ERRO: $($_.Exception.Message) @ $(($_.ScriptStackTrace -split "`n")[0])" }
}

try {
  Run 'B8-M1-manual-antes-do-predecessor' {
    param($p)
    $a = Add-Task $p 'A' 3
    $b = Manual $p 'B_manual' '2026-11-03 09:00' 2; Link $a $b; $b.Start = [datetime]'2026-11-03 09:00'
    $c = Add-Task $p 'C_apos_B' 1; Link $b $c
  }
  Run 'B8-M2-manual-depois' {
    param($p)
    $a = Add-Task $p 'A' 2
    $b = Manual $p 'B_manual' '2026-11-10 09:00' 2; Link $a $b; $b.Start = [datetime]'2026-11-10 09:00'
    $c = Add-Task $p 'C_apos_B' 1; Link $b $c
    [void](Add-Task $p 'D_paralela' 3)
  }
  Run 'B8-M3-manual-sem-vinculos' {
    param($p)
    [void](Manual $p 'M_solta' '2026-11-04 09:00' 3)
    $a = Add-Task $p 'A' 2; $b = Add-Task $p 'B' 1; Link $a $b
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
