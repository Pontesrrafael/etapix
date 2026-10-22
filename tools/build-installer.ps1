# Builds etapix and its installer in release and copies the installer to dist\.
# Needs the GNU Rust toolchain (rust-toolchain.toml) and the portable WinLibs binutils in
# %USERPROFILE%\tools\mingw64 (their `as` is what dlltool needs for the Windows UI crates).
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:USERPROFILE\tools\mingw64\bin;$env:Path"
Push-Location $root
try {
  cargo build -p etapix --release
  if ($LASTEXITCODE) { throw 'falha ao compilar o Etapix' }
  # The connector and its Claude Desktop extension go inside the installer.
  & (Join-Path $PSScriptRoot 'build-integrations.ps1')
  cargo build -p etapix-setup --release
  if ($LASTEXITCODE) { throw 'falha ao compilar o instalador' }
  $version = (Select-String -Path setup\Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
  New-Item -ItemType Directory -Force dist | Out-Null
  $out = "dist\etapix-setup-$version.exe"
  Copy-Item target\release\etapix-setup.exe $out -Force
  # A copy at the top of the repository, for downloading straight from GitHub.
  Copy-Item $out "Etapix-Instalador.exe" -Force
  "instalador: $(Resolve-Path $out)"
} finally {
  Pop-Location
}
