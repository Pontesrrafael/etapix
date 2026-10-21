param([string]$OutDir, [string]$Only = '')
# Batch 14: every constraint type on a summary task. CPM only; each case exports one `<case>-antes.xml`.
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
  foreach ($c in @(@{n='ALAP';t=1;d=''}, @{n='MSO';t=2;d='2026-11-05 09:00'}, @{n='MFO';t=3;d='2026-11-10 18:00'}, @{n='SNLT';t=5;d='2026-11-04 09:00'}, @{n='FNET';t=6;d='2026-11-12 18:00'}, @{n='FNLT';t=7;d='2026-11-05 18:00'})) {
    Run "B14-S-$($c.n)" {
      param($p)
      $x = Add-Task $p 'X_antes' 2
      $s = Add-Task $p 'S' 1
      $a = Add-Task $p 'A' 2; $a.OutlineIndent()
      $b = Add-Task $p 'B' 1; Link $a $b
      $y = Add-Task $p 'Y_paralela' 8; $y.OutlineOutdent()
      Link $x $s
      $s.ConstraintType = $c.t
      if ($c.d) { $s.ConstraintDate = [datetime]$c.d }
    }
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
