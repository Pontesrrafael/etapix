param([string]$InDir, [string]$OutDir)
# Opens every MSPDI file pjsched wrote (examples/roundtrip_write.rs), lets Project recalculate,
# and saves Project's version to $OutDir for tests/roundtrip.rs to compare.
$ErrorActionPreference = 'Stop'
$M = [System.Reflection.Missing]::Value
$DO_NOT_OPEN_POOL = 4   # PjPoolOpen

New-Item -ItemType Directory -Force $OutDir | Out-Null
$app = New-Object -ComObject MSProject.Application
$app.Visible = $false
$app.DisplayAlerts = $false
"Project version: $($app.Version) build $($app.Build)"
try {
  foreach ($f in Get-ChildItem $InDir -Filter *.xml | Sort-Object Name) {
    try {
      # FileOpenEx(Name, ReadOnly, Merge, TaskInformation, Table, Sheet, NoAuto, UserID,
      #            DatabasePassWord, FormatID, Map, openPool, ...)
      $open = @($f.FullName, $true) + @($M) * 7 + @('MSProject.XML', $M, $DO_NOT_OPEN_POOL) + @($M) * 5
      [void]$app.FileOpenEx.Invoke($open)
      [void]$app.CalculateProject()
      $save = @((Join-Path $OutDir $f.Name), 0) + @($M) * 7 + @('MSProject.XML') + @($M) * 9
      [void]$app.FileSaveAs.Invoke($save)
      [void]$app.FileCloseEx(0, $M, $M)
      "  ok $($f.Name)"
    } catch {
      "  ERRO $($f.Name): $($_.Exception.Message)"
      try { [void]$app.FileCloseEx(0, $M, $M) } catch { }
    }
  }
}
finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
