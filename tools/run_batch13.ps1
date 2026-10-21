param([string]$OutDir, [string]$Only = '')
# Batch 13: monthly and yearly recurring calendar exceptions. CPM only; each case exports one `<case>-antes.xml`.
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
# PjExceptionType: 2 yearly by month day, 3 yearly positional, 4 monthly by day, 5 monthly positional.
# PjExceptionItem for MonthItem: day of week; PjExceptionPosition for MonthPosition.
function Exc($p, [int]$type, [string]$from, [string]$to) {
  $e = $p.Calendar.Exceptions.Add($type, [datetime]$from, [datetime]$to)
  Write-Output -NoEnumerate $e
}
function LongChain($p) { $a = Add-Task $p 'A' 70; $b = Add-Task $p 'B' 2; Link $a $b }

try {
  Run 'B13-E1-mensal-dia-10' {
    param($p)
    $e = Exc $p 4 '2026-11-10' '2027-01-10'; $e.MonthDay = 10; $e.Period = 1
    LongChain $p
  }
  Run 'B13-E2-mensal-primeira-segunda' {
    param($p)
    $e = Exc $p 5 '2026-12-01' '2027-02-28'; $e.MonthPosition = 0; $e.MonthItem = 4; $e.Period = 1
    LongChain $p
  }
  Run 'B13-E3-anual-25-dez' {
    param($p)
    $e = Exc $p 2 '2026-12-25' '2027-12-25'; $e.Month = 12; $e.MonthDay = 25
    LongChain $p
  }
  Run 'B13-E4-anual-ultima-sexta-nov' {
    param($p)
    $e = Exc $p 3 '2026-11-27' '2027-11-26'; $e.Month = 11; $e.MonthPosition = 4; $e.MonthItem = 8
    LongChain $p
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
