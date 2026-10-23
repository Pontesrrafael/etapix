param([string]$OutDir = (Join-Path (Split-Path $PSScriptRoot -Parent) 'tests\mpp-corpus\var'))
# Controlled variations for the native .mpp reader: small projects built through Project's COM
# model where one aspect changes per file (flags, baseline, resource kinds, links, calendars,
# project options). Each is saved as .mpp and as Project XML (the truth).
$ErrorActionPreference = 'Stop'
$M = [System.Reflection.Missing]::Value
New-Item -ItemType Directory -Force $OutDir | Out-Null
$running = @(Get-Process WINPROJ -ErrorAction SilentlyContinue).Count -gt 0
$app = New-Object -ComObject MSProject.Application
$alerts = $app.DisplayAlerts
$app.DisplayAlerts = $false

function Save($name) {
  $base = Join-Path $OutDir $name
  [void]$app.CalculateProject()
  [void]$app.FileSaveAs.Invoke(@("$base.mpp", 0) + @($M) * 17)
  [void]$app.FileSaveAs.Invoke(@("$base.xml", 0) + @($M) * 7 + @('MSProject.XML') + @($M) * 9)
  [void]$app.FileCloseEx(0, $M, $M)
  "  $name"
}

function NewProject([datetime]$start) {
  $p = $app.Projects.Add()
  $p.ProjectStart = $start
  # New tasks are auto-scheduled (the installation may default to manual).
  $p.NewTasksCreatedAsManual = $false
  $p
}
# Elapsed durations as the installation's own text ("3 diasd" in pt-BR); pjElapsedDays = 8.
function Elapsed([int]$days) { $app.DurationFormat($days * 1440, 8) }

try {
  if (-not (Test-Path (Join-Path $OutDir 'V1-flags.mpp'))) {
  # 1. Task flags, alternating over 8 tasks.
  $p = NewProject ([datetime]'2026-11-02 08:00')
  for ($i = 1; $i -le 8; $i++) {
    $t = $p.Tasks.Add("T$i")
    $t.Duration = 480 * $i
    $t.EffortDriven = ($i % 2 -eq 0)
    $t.LevelingCanSplit = ($i % 3 -ne 0)
    $t.FixedCost = 100 * $i + 0.25
    $t.FixedCostAccrual = ($i % 3) + 1
    $t.Priority = 100 * $i
    if ($i -in 3, 6) { $t.Manual = $true }
    if ($i -eq 7) { $t.Duration = [string](Elapsed 3) }
    if ($i -eq 8) { $t.Duration = 0 }
  }
  Save 'V1-flags'

  # 2. Baseline: plan, save baseline, then change durations.
  $p = NewProject ([datetime]'2026-11-02 08:00')
  for ($i = 1; $i -le 5; $i++) { $t = $p.Tasks.Add("B$i"); $t.Duration = 480 * $i }
  $p.Tasks.Item(2).Predecessors = '1'
  $p.Tasks.Item(3).Predecessors = '2'
  [void]$app.BaselineSave($true)
  for ($i = 1; $i -le 5; $i++) { $p.Tasks.Item($i).Duration = 480 * ($i + 1) }
  Save 'V2-baseline'

  # 3. Links of every type, with lags (working, elapsed, negative).
  $p = NewProject ([datetime]'2026-11-02 08:00')
  for ($i = 1; $i -le 8; $i++) { $t = $p.Tasks.Add("L$i"); $t.Duration = 960 }
  $p.Tasks.Item(2).Predecessors = '1'
  $p.Tasks.Item(3).Predecessors = '1II+2d'
  $p.Tasks.Item(4).Predecessors = '2TT-1d'
  $p.Tasks.Item(5).Predecessors = '3IT+1d'
  $p.Tasks.Item(6).Predecessors = "4;5TI+$(Elapsed 3)"
  $p.Tasks.Item(7).Predecessors = '6TI+2d'
  $p.Tasks.Item(8).Predecessors = '1;2;3'
  Save 'V3-links'

  # 4. Resources of each kind, with units, rates and an assignment each.
  $p = NewProject ([datetime]'2026-11-02 08:00')
  $kinds = @(0, 0, 1, 2)   # pjResourceTypeWork, Work, Material, Cost
  for ($i = 1; $i -le 4; $i++) {
    $r = $p.Resources.Add("R$i")
    $r.Type = $kinds[$i - 1]
    if ($kinds[$i - 1] -eq 0) { $r.MaxUnits = 0.5 * $i; $r.StandardRate = "$(10 * $i)/h"; $r.OvertimeRate = "$(15 * $i)/h" }
    if ($kinds[$i - 1] -eq 1) { $r.StandardRate = 7; $r.MaterialLabel = 'kg' }
    if ($kinds[$i - 1] -ne 2) { $r.CostPerUse = 3 * $i }
  }
  for ($i = 1; $i -le 4; $i++) {
    $t = $p.Tasks.Add("A$i"); $t.Duration = 960 * $i
    $a = $t.Assignments.Add($t.ID, $i)
    if ($i -eq 1) { $a.Units = 0.75 }
    if ($i -eq 3) { $a.Units = 12 }
    if ($i -eq 4) { $a.Cost = 250 }
  }
  Save 'V4-resources'

  # 5. Calendars: a task calendar and a resource calendar with exceptions and odd hours.
  $p = NewProject ([datetime]'2026-11-02 08:00')
  [void]$app.BaseCalendarCreate('Turno', $p.Calendar.Name)
  $cal = $p.BaseCalendars.Item('Turno')
  $mon = $cal.WeekDays.Item(2)
  $mon.Shift1.Start = '07:00'; $mon.Shift1.Finish = '11:00'
  $mon.Shift2.Start = '12:00'; $mon.Shift2.Finish = '15:00'
  $sat = $cal.WeekDays.Item(7)
  $sat.Working = $true
  $sat.Shift1.Start = '08:00'; $sat.Shift1.Finish = '12:00'
  [void]$cal.Exceptions.Add(1, [datetime]'2026-11-10', [datetime]'2026-11-11')
  $e = $cal.Exceptions.Add(1, [datetime]'2026-11-16', [datetime]'2026-11-16')
  $e.Shift1.Start = '13:00'; $e.Shift1.Finish = '17:00'
  $t = $p.Tasks.Add('C1'); $t.Duration = 2400; $t.Calendar = 'Turno'
  $r = $p.Resources.Add('RC')
  $rc = $r.Calendar
  [void]$rc.Exceptions.Add(1, [datetime]'2026-11-04', [datetime]'2026-11-05')
  $t2 = $p.Tasks.Add('C2'); $t2.Duration = 1440
  [void]$t2.Assignments.Add($t2.ID, $r.ID)
  Save 'V5-calendars'
  }

  # 7. One flag per file, on tasks 2 and 5 only, so each flag's bit stands alone.
  foreach ($flag in 'Manual', 'EffortDriven', 'LevelingCanSplit', 'Estimated', 'HideBar', 'Rollup') {
    if (Test-Path (Join-Path $OutDir "V7-$flag.mpp")) { continue }
    $p = NewProject ([datetime]'2026-11-02 08:00')
    for ($i = 1; $i -le 6; $i++) {
      $t = $p.Tasks.Add("F$i"); $t.Duration = 960
      $on = $i -in 2, 5
      switch ($flag) {
        'LevelingCanSplit' { $t.LevelingCanSplit = -not $on }
        default { if ($on) { $t.$flag = $true } }
      }
    }
    Save "V7-$flag"
  }

  # 8. Project calendar other than the default one (and two more base calendars before it).
  if (-not (Test-Path (Join-Path $OutDir 'V8-project-calendar.mpp'))) {
    $p = NewProject ([datetime]'2026-11-02 08:00')
    [void]$app.BaseCalendarCreate('Extra1', $p.Calendar.Name)
    [void]$app.BaseCalendarCreate('Extra2', $p.Calendar.Name)
    [void]$app.BaseCalendarCreate('Noturno', $p.Calendar.Name)
    $cal = $p.BaseCalendars.Item('Noturno')
    foreach ($d in 2..6) { $cal.WeekDays.Item($d).Shift2.Clear() }
    # ProjectSummaryInfo(Project, Title, Subject, Author, Company, Manager, Keywords, Comments,
    #                    Start, Finish, ScheduleFrom, CurrentDate, Calendar, ...)
    [void]$app.ProjectSummaryInfo($M, $M, $M, $M, $M, $M, $M, $M, $M, $M, $M, $M, 'Noturno')
    for ($i = 1; $i -le 3; $i++) { $t = $p.Tasks.Add("N$i"); $t.Duration = 960 }
    Save 'V8-project-calendar'
  }

  # 9. A resource whose rates change over time (cost rate table A with three periods).
  if (-not (Test-Path (Join-Path $OutDir 'V9-rate-table.mpp'))) {
    $p = NewProject ([datetime]'2026-11-02 08:00')
    $r = $p.Resources.Add('RT')
    $r.StandardRate = '10/h'; $r.OvertimeRate = '15/h'; $r.CostPerUse = 3
    $rates = $r.CostRateTables.Item(1).PayRates
    [void]$rates.Add([datetime]'2026-11-05', '20/h', '25/h', 4)
    [void]$rates.Add([datetime]'2026-11-10', '30/h', '35/h', 5)
    $t = $p.Tasks.Add('X'); $t.Duration = 4800
    [void]$t.Assignments.Add($t.ID, $r.ID)
    Save 'V9-rate-table'
  }

  # 6. Project options.
  if (-not (Test-Path (Join-Path $OutDir 'V6-options.mpp'))) {
  $p = NewProject ([datetime]'2026-12-07 09:30')
  $p.StatusDate = [datetime]'2026-12-15 17:00'
  for ($i = 1; $i -le 3; $i++) { $t = $p.Tasks.Add("P$i"); $t.Duration = 480 }
  Save 'V6-options'
  }
} finally {
  $app.DisplayAlerts = $alerts
  if (-not $running) { [void]$app.Quit(0) }
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
