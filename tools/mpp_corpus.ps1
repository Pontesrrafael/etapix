param([string]$OutDir = (Join-Path (Split-Path $PSScriptRoot -Parent) 'tests\mpp-corpus'))
# Builds the corpus for Etapix's native .mpp reader: every golden MSPDI file is opened in the
# installed Microsoft Project, recalculated, and saved twice: as .mpp (what the reader reads) and
# as Project XML (what it must find in it). Both come from the same Project session, so the XML
# is the truth for the .mpp next to it.
$ErrorActionPreference = 'Stop'
$M = [System.Reflection.Missing]::Value
$root = Split-Path $PSScriptRoot -Parent
New-Item -ItemType Directory -Force $OutDir | Out-Null
$running = @(Get-Process WINPROJ -ErrorAction SilentlyContinue).Count -gt 0
$app = New-Object -ComObject MSProject.Application
$alerts = $app.DisplayAlerts
$app.DisplayAlerts = $false
"Project $($app.Version) build $($app.Build)"
$n = 0
try {
  foreach ($f in Get-ChildItem (Join-Path $root 'tests\golden') -Recurse -Filter *-antes.xml | Sort-Object FullName) {
    $batch = $f.Directory.Name
    $dir = Join-Path $OutDir $batch
    New-Item -ItemType Directory -Force $dir | Out-Null
    $base = Join-Path $dir ($f.BaseName -replace '-antes$', '')
    try {
      $open = @($f.FullName, $true) + @($M) * 7 + @('MSProject.XML', $M, 4) + @($M) * 5
      [void]$app.FileOpenEx.Invoke($open)
      [void]$app.CalculateProject()
      [void]$app.FileSaveAs.Invoke(@("$base.mpp", 0) + @($M) * 17)
      [void]$app.FileSaveAs.Invoke(@("$base.xml", 0) + @($M) * 7 + @('MSProject.XML') + @($M) * 9)
      [void]$app.FileCloseEx(0, $M, $M)
      $n++
    } catch {
      "  ERRO $($f.Name): $($_.Exception.Message)"
      try { [void]$app.FileCloseEx(0, $M, $M) } catch { }
    }
  }
} finally {
  $app.DisplayAlerts = $alerts
  if (-not $running) { [void]$app.Quit(0) }
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
# Keep Project's recent-files lists free of the corpus.
$mru = 'HKCU:\Software\Microsoft\Office\16.0\MS Project\User MRU'
if (Test-Path $mru) {
  foreach ($list in Get-ChildItem $mru -Recurse | Where-Object { $_.PSChildName -in 'File MRU', 'Place MRU' }) {
    $names = $list.GetValueNames() | Where-Object { $_ -match '^Item (\d+)$' } | Sort-Object { [int]($_ -replace '\D', '') }
    $keep = foreach ($k in $names) {
      $v = $list.GetValue($k)
      if ($v -notlike "*mpp-corpus*" -and $v -notlike "*tests\golden*") { [pscustomobject]@{ Item = $v; Meta = $list.GetValue(($k -replace 'Item ', 'Item Metadata ')) } }
    }
    if (@($keep).Count -eq @($names).Count) { continue }
    foreach ($k in ($list.GetValueNames() | Where-Object { $_ -match '^Item (Metadata )?\d+$' })) { Remove-ItemProperty -Path $list.PSPath -Name $k }
    $i = 1
    foreach ($e in $keep) {
      New-ItemProperty -Path $list.PSPath -Name "Item $i" -Value $e.Item -PropertyType String | Out-Null
      if ($null -ne $e.Meta) { New-ItemProperty -Path $list.PSPath -Name "Item Metadata $i" -Value $e.Meta -PropertyType String | Out-Null }
      $i++
    }
  }
}
"$n arquivos em $OutDir"
