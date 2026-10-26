# Builds the Etapix connector for Claude and Codex and packs it in dist\:
#   etapix.mcpb                       Claude Desktop extension (double-click installs it)
#   etapix-skill.zip                  the skill, for claude.ai's "upload skill"
#   Etapix-Claude-Codex-<version>.zip everything above plus instalar.cmd (one double-click)
# Needs the same PATH as build-installer.ps1 (Rust and the WinLibs binutils).
param([string]$CertificateThumbprint, [switch]$ForPublication)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
# Compress-Archive in Windows PowerShell writes "\" inside entry names, which other unzippers
# (Claude Desktop's among them) do not treat as folders; this writes "/" as the zip format asks.
function Zip-Folder($from, $to, [switch]$KeepTop) {
  Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
  if (Test-Path $to) { Remove-Item $to -Force }
  $base = if ($KeepTop) { Split-Path -Parent (Resolve-Path $from) } else { (Resolve-Path $from).Path }
  $zip = [IO.Compression.ZipFile]::Open($to, 'Create')
  try {
    foreach ($f in Get-ChildItem $from -Recurse -File) {
      $name = $f.FullName.Substring($base.Length).TrimStart('\').Replace('\', '/')
      [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $f.FullName, $name, 'Optimal')
    }
  } finally { $zip.Dispose() }
}

try {
  $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:USERPROFILE\tools\mingw64\bin;$env:PATH"
  cargo build -p etapix --release --bin etapix-mcp
  if ($LASTEXITCODE) { throw 'falha ao compilar o etapix-mcp' }
  & (Join-Path $PSScriptRoot 'sign-artifacts.ps1') -Paths @('target/release/etapix-mcp.exe') -CertificateThumbprint $CertificateThumbprint -RequireSignature:$ForPublication
  $version = (Get-Content integrations\mcpb\manifest.json -Raw | ConvertFrom-Json).version
  $work = Join-Path $root 'target\integrations'
  Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force "$work\mcpb\server", "$work\pacote", 'dist' | Out-Null

  # Claude Desktop extension: manifest, icon and the server binary, zipped as .mcpb.
  Copy-Item integrations\mcpb\manifest.json "$work\mcpb\"
  cargo run -q -p etapix --release --example logo_png -- "$work\mcpb\icon.png" 256
  if ($LASTEXITCODE) { throw 'falha ao gerar o ícone' }
  Copy-Item target\release\etapix-mcp.exe "$work\mcpb\server\"
  Zip-Folder "$work\mcpb" (Join-Path $root 'dist\etapix.mcpb')

  # Skill for claude.ai: a zip whose top folder is the skill.
  Zip-Folder integrations\skill\etapix (Join-Path $root 'dist\etapix-skill.zip') -KeepTop

  # The one-double-click package.
  $p = "$work\pacote"
  Copy-Item dist\etapix.mcpb, dist\etapix-skill.zip, target\release\etapix-mcp.exe, integrations\instalar.cmd, integrations\LEIA-ME.txt $p
  New-Item -ItemType Directory -Force "$p\skill" | Out-Null
  Copy-Item integrations\skill\etapix "$p\skill\" -Recurse
  # Windows PowerShell 5.1 reads scripts without a BOM as ANSI: write it with one.
  $ps = Get-Content integrations\instalar.ps1 -Raw -Encoding UTF8
  [IO.File]::WriteAllText("$p\instalar.ps1", $ps, (New-Object Text.UTF8Encoding $true))
  $out = "dist\Etapix-Claude-Codex-$version.zip"
  Zip-Folder $p (Join-Path $root $out)
  Write-Host "pacote: $(Resolve-Path $out)"
} finally {
  Pop-Location
}
