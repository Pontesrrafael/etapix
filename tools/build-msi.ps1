param([string]$Version='0.1.1', [string]$CertificateThumbprint, [switch]$ForPublication)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
  $wix=Join-Path $root 'target/wix-tools/wix.exe'
  if (-not(Test-Path -LiteralPath $wix)) { throw 'Instale a ferramenta local: dotnet tool install wix --version 4.0.6 --tool-path target/wix-tools' }
  $stage=Join-Path $root 'target/msi-payload'
  New-Item -ItemType Directory -Force $stage | Out-Null
  foreach($name in @('etapix.exe','etapix-mcp.exe')) {
    Copy-Item -LiteralPath (Join-Path $root "target/release/$name") -Destination (Join-Path $stage $name) -Force
  }
  foreach($name in @('LICENSE-MIT','LICENSE-APACHE')) { Copy-Item -LiteralPath (Join-Path $root $name) -Destination (Join-Path $stage "$name.txt") -Force }
  Copy-Item -LiteralPath (Join-Path $root 'THIRD-PARTY-NOTICES.txt') -Destination $stage -Force
  $readme = [IO.File]::ReadAllText((Join-Path $root 'packaging/windows/LEIA-ME-MSI.txt'))
  [IO.File]::WriteAllText((Join-Path $stage 'LEIA-ME-MSI.txt'), $readme + "`nVersao do pacote MSI: $Version`n", [Text.UTF8Encoding]::new($false))
  & ./tools/sign-artifacts.ps1 -Paths @("$stage/etapix.exe","$stage/etapix-mcp.exe") -CertificateThumbprint $CertificateThumbprint -RequireSignature:$ForPublication | Out-Host
  $out=Join-Path $root "dist/Etapix-$Version-x64.msi"
  & $wix build packaging/windows/Etapix.wxs -arch x64 -d "Version=$Version" -d "Payload=$stage" -out $out
  if ($LASTEXITCODE) { throw 'Falha ao gerar MSI' }
  & ./tools/sign-artifacts.ps1 -Paths @($out) -CertificateThumbprint $CertificateThumbprint -RequireSignature:$ForPublication | Out-Host
  $hash=Get-FileHash -LiteralPath $out
  [IO.File]::WriteAllText("$out.sha256", "$($hash.Hash)  $([IO.Path]::GetFileName($out))`n", [Text.UTF8Encoding]::new($false))
  Write-Output $out
} finally { Pop-Location }
