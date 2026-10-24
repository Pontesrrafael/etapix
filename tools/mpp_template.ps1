# Builds assets/modelo.mpp with the installed Microsoft Project: the template Etapix creates new
# .mpp files from. Besides the project summary it holds one record of each kind the writer clones:
# a task with a resource (T1), a task without one (T2, with Project's placeholder assignment), a
# link T1 -> T2, a work resource (R1, with its calendar) and the assignment of R1 to T1.
param([string]$Out = (Join-Path $PSScriptRoot '..\assets\modelo.mpp'))
$ErrorActionPreference = 'Stop'
$Out = [IO.Path]::GetFullPath($Out)
$M = [System.Reflection.Missing]::Value
$app = New-Object -ComObject MSProject.Application
$app.DisplayAlerts = $false
try {
  [void]$app.FileNew($M, $M, $M, $false)
  $p = $app.ActiveProject
  $p.NewTasksCreatedAsManual = $false
  $p.ProjectStart = '02/11/2026 08:00'
  $t1 = $p.Tasks.Add('T1'); $t1.Duration = 480
  $t2 = $p.Tasks.Add('T2'); $t2.Duration = 480
  $t2.LinkPredecessors($t1)
  [void]$p.Resources.Add('R1')
  [void]$t1.Assignments.Add($t1.ID, 1)
  [void]$app.FileSaveAs.Invoke(@($Out, 0) + @($M) * 17)
  [void]$app.FileCloseEx(0, $M, $M)
} finally { [void]$app.Quit(0) }
"modelo: $Out"
