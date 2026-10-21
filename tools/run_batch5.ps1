param([string]$OutDir, [string]$Only = '')
# Batch 5: task calendars, resource calendars, exceptions with working hours and elapsed
# durations/lags. CPM only; each case exports one `<case>-antes.xml`.
$ErrorActionPreference = 'Stop'
$M = [System.Reflection.Missing]::Value
$DAY = 480
$Start = [datetime]'2026-11-02 08:00'   # Monday
$FS = 1; $SNET = 4
$DAILY = 1                                # PjExceptionType
$SUNDAY = 1; $SATURDAY = 7                # PjWeekday
# Elapsed days as the installation's own duration text ("3 diasd" in pt-BR); pjElapsedDays = 8.
function Elapsed([int]$days) { $app.DurationFormat($days * 1440, 8) }

$app = New-Object -ComObject MSProject.Application
$app.Visible = $false
$app.DisplayAlerts = $false
"Project version: $($app.Version) build $($app.Build)"

function Add-Task($p, [string]$name, $duration) {
  $t = $p.Tasks.Add($name)
  $t.Manual = $false
  if ($duration -is [string]) { $t.Duration = [string]$duration } else { $t.Duration = $duration * $DAY }
  Write-Output -NoEnumerate $t
}
function Link($pred, $succ, $lag = 0) { [void]$succ.TaskDependencies.Add($pred, $FS, $lag) }

# Standard week plus Saturday and Sunday with the same hours.
function New-SevenDayCalendar {
  [void]$app.BaseCalendarCreate('Sete-dias', $app.ActiveProject.Calendar.Name)
  $cal = $app.ActiveProject.BaseCalendars.Item('Sete-dias')
  foreach ($d in $SUNDAY, $SATURDAY) {
    $day = $cal.WeekDays.Item($d)
    $day.Working = $true
    $day.Shift1.Start = '09:00'; $day.Shift1.Finish = '12:00'
    $day.Shift2.Start = '13:00'; $day.Shift2.Finish = '18:00'
  }
}
# Mornings only: 09:00-12:00 on weekdays (3h/day).
function New-MorningCalendar($p) {
  [void]$app.BaseCalendarCreate('Meio-periodo', $p.Calendar.Name)
  $cal = $p.BaseCalendars.Item('Meio-periodo')
  foreach ($d in 2..6) { $cal.WeekDays.Item($d).Shift2.Clear() }
}
function Add-Exception($calendar, [string]$date, [string]$from = '', [string]$to = '') {
  $e = $calendar.Exceptions.Add($DAILY, [datetime]$date, [datetime]$date)
  if ($from) { $e.Shift1.Start = $from; $e.Shift1.Finish = $to }
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

try {
  Run 'B5-C1-cal-tarefa-7dias' {
    param($p)
    New-SevenDayCalendar
    $a = Add-Task $p 'A_7dias_6d' 6; $a.Calendar = 'Sete-dias'
    $b = Add-Task $p 'B' 2; Link $a $b
    [void](Add-Task $p 'E_solta' 1)
  }
  Run 'B5-C2-cal-tarefa-meio' {
    param($p)
    New-MorningCalendar $p
    $a = Add-Task $p 'A_meio_2d' 2; $a.Calendar = 'Meio-periodo'
    $b = Add-Task $p 'B' 2; Link $a $b
  }
  Run 'B5-C3-recurso-excecao' {
    param($p)
    $r = $p.Resources.Add('R1'); Add-Exception $r.Calendar '2026-11-04'
    $t = Add-Task $p 'T_R1_3d' 3; [void]$t.Assignments.Add($t.ID, $r.ID, 1)
    $u = Add-Task $p 'U_sem_recurso_3d' 3
    $v = Add-Task $p 'V_apos_T' 1; Link $t $v
  }
  Run 'B5-C4-recurso-meio' {
    param($p)
    New-MorningCalendar $p
    $r = $p.Resources.Add('R2'); $r.BaseCalendar = 'Meio-periodo'
    $t = Add-Task $p 'T_R2_2d' 2; [void]$t.Assignments.Add($t.ID, $r.ID, 1)
    $v = Add-Task $p 'V_apos_T' 1; Link $t $v
  }
  Run 'B5-C5-tarefa-e-recurso' {
    param($p)
    New-SevenDayCalendar
    $r = $p.Resources.Add('R1'); Add-Exception $r.Calendar '2026-11-04'
    $t = Add-Task $p 'T_7dias_R1_6d' 6; $t.Calendar = 'Sete-dias'
    [void]$t.Assignments.Add($t.ID, $r.ID, 1)
  }
  Run 'B5-C6-ignora-recurso' {
    param($p)
    New-SevenDayCalendar
    $r = $p.Resources.Add('R1'); Add-Exception $r.Calendar '2026-11-04'
    $t = Add-Task $p 'T_7dias_R1_6d' 6; $t.Calendar = 'Sete-dias'; $t.IgnoreResourceCalendar = $true
    [void]$t.Assignments.Add($t.ID, $r.ID, 1)
  }
  Run 'B5-C7-excecao-sabado' {
    param($p)
    Add-Exception $p.Calendar '2026-11-07' '09:00' '13:00'
    $a = Add-Task $p 'A_6d' 6
    $b = Add-Task $p 'B' 1; Link $a $b
  }
  Run 'B5-C8-excecao-parcial' {
    param($p)
    Add-Exception $p.Calendar '2026-11-03' '09:00' '12:00'
    $a = Add-Task $p 'A_3d' 3
    $b = Add-Task $p 'B' 1; Link $a $b
  }
  Run 'B5-C9-decorrido' {
    param($p)
    $a = Add-Task $p 'A_3ed' (Elapsed 3)
    $b = Add-Task $p 'B_apos_A' 2; Link $a $b
    $c = Add-Task $p 'C_lag_2ed' 1; Link $a $c (Elapsed 2)
    $d = Add-Task $p 'D_2ed_sexta' (Elapsed 2); $d.ConstraintType = $SNET; $d.ConstraintDate = [datetime]'2026-11-06 09:00'
    $e = Add-Task $p 'E_apos_D' 1; Link $d $e
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}

