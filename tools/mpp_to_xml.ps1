# Opens every .mpp in a folder in the installed Microsoft Project, hidden, and saves each as
# NAME.proj.xml next to it (used by examples/mpp_write_check.rs). Files that Project cannot
# open are reported. Project's recent-file list is cleaned of these files afterwards.
param([Parameter(Mandatory)][string]$Dir)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$Dir = (Resolve-Path $Dir).Path
$M = [System.Reflection.Missing]::Value
$app = New-Object -ComObject MSProject.Application
$app.DisplayAlerts = $false
try {
  foreach ($f in Get-ChildItem $Dir -Filter *.mpp) {
    $xml = Join-Path $Dir ($f.BaseName + '.proj.xml')
    Remove-Item $xml -ErrorAction SilentlyContinue
    try {
      # FileOpenEx(Name, ReadOnly, ..., openPool = pjDoNotOpenPool); FileSaveAs(Name, ..., FormatID)
      [void]$app.FileOpenEx.Invoke(@($f.FullName, $true) + @($M) * 7 + @($M, $M, 4) + @($M) * 5)
      [void]$app.FileSaveAs.Invoke(@($xml, 0) + @($M) * 7 + @('MSProject.XML') + @($M) * 9)
      [void]$app.FileCloseEx(0, $M, $M)
    } catch {
      "$($f.Name): $($_.Exception.Message)"
    }
  }
} finally {
  [void]$app.Quit(0)
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app)
}
$root = 'HKCU:\Software\Microsoft\Office\16.0\MS Project\User MRU'
if (Test-Path $root) {
  foreach ($list in Get-ChildItem $root -Recurse | Where-Object { $_.PSChildName -in 'File MRU', 'Place MRU' }) {
    $names = $list.GetValueNames() | Where-Object { $_ -match '^Item (\d+)$' } | Sort-Object { [int]($_ -replace '\D', '') }
    $keep = foreach ($n in $names) {
      $v = $list.GetValue($n)
      if ($v -notlike "*$Dir*") { [pscustomobject]@{ Item = $v; Meta = $list.GetValue(($n -replace 'Item ', 'Item Metadata ')) } }
    }
    if (@($keep).Count -eq @($names).Count) { continue }
    foreach ($nm in ($list.GetValueNames() | Where-Object { $_ -match '^Item (Metadata )?\d+$' })) { Remove-ItemProperty -Path $list.PSPath -Name $nm }
    $i = 1
    foreach ($e in $keep) {
      New-ItemProperty -Path $list.PSPath -Name "Item $i" -Value $e.Item -PropertyType String | Out-Null
      if ($null -ne $e.Meta) { New-ItemProperty -Path $list.PSPath -Name "Item Metadata $i" -Value $e.Meta -PropertyType String | Out-Null }
      $i++
    }
  }
}
