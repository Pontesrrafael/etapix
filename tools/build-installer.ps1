# Builds pjapp and its installer in release and copies the installer to dist\.
# Needs the GNU Rust toolchain (rust-toolchain.toml) and the portable WinLibs binutils in
# %USERPROFILE%\tools\mingw64 (their `as` is what dlltool needs for the Windows UI crates).
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:USERPROFILE\tools\mingw64\bin;$env:Path"
Push-Location $root
try {
  cargo build -p pjapp --release
  if ($LASTEXITCODE) { throw 'falha ao compilar o pjapp' }
  cargo build -p pjapp-setup --release
  if ($LASTEXITCODE) { throw 'falha ao compilar o instalador' }
  $version = (Select-String -Path setup\Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
  New-Item -ItemType Directory -Force dist | Out-Null
  $out = "dist\pjapp-setup-$version.exe"
  Copy-Item target\release\pjapp-setup.exe $out -Force
  "instalador: $(Resolve-Path $out)"
} finally {
  Pop-Location
}
