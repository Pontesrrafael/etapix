param([Parameter(Mandatory=$true)][string]$Package,
      [Parameter(Mandatory=$true)][string]$UpgradePackage)
$ErrorActionPreference='Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') { throw 'Execute somente no runner Windows descartável do GitHub Actions.' }
$installer=New-Object -ComObject WindowsInstaller.Installer
function Read-Property($path,$property) {
  $db=$installer.OpenDatabase((Resolve-Path -LiteralPath $path).Path,0)
  $view=$db.OpenView("SELECT ``Value`` FROM ``Property`` WHERE ``Property``='$property'")
  $view.Execute(); $record=$view.Fetch(); $value=$record.StringData(1); $view.Close(); return $value
}
function Assert-NoCustomActions($path) {
  $db=$installer.OpenDatabase((Resolve-Path -LiteralPath $path).Path,0)
  $view=$db.OpenView('SELECT `Name` FROM `_Tables` WHERE `Name`=''CustomAction''')
  $view.Execute(); if($view.Fetch()) { throw 'CustomAction não permitida no MSI' }; $view.Close()
}
function Run-Msi([string[]]$MsiArguments) {
  $process=Start-Process -FilePath msiexec.exe -ArgumentList $MsiArguments -Wait -PassThru -WindowStyle Hidden
  if($process.ExitCode -notin @(0,3010)) { throw "msiexec retornou $($process.ExitCode): $MsiArguments" }
}
$Package=(Resolve-Path -LiteralPath $Package).Path
$UpgradePackage=(Resolve-Path -LiteralPath $UpgradePackage).Path
Assert-NoCustomActions $Package
Assert-NoCustomActions $UpgradePackage
$product=Read-Property $Package 'ProductCode'
$upgradeProduct=Read-Property $UpgradePackage 'ProductCode'
if($product -eq $upgradeProduct) { throw 'ProductCode não foi renovado' }
if((Read-Property $Package 'UpgradeCode') -ne (Read-Property $UpgradePackage 'UpgradeCode')) { throw 'UpgradeCode divergente' }
$dir=Join-Path $env:LOCALAPPDATA 'Programs/Etapix-MSI'
if(Test-Path -LiteralPath $dir) { throw 'Runner já contém instalação; isolamento inválido' }
$logDir=Join-Path $env:GITHUB_WORKSPACE 'dist/lifecycle-logs'
New-Item -ItemType Directory -Force $logDir | Out-Null
$clients=@("$env:USERPROFILE/.claude.json","$env:USERPROFILE/.codex/config.toml")
$before=@{}
foreach($file in $clients) { $before[$file]=if(Test-Path -LiteralPath $file) {(Get-FileHash -LiteralPath $file).Hash} else {'ABSENT'} }
Run-Msi @('/i',"`"$Package`"",'/qn','/norestart','/L*v',"`"$logDir/install.log`"")
foreach($name in @('etapix.exe','etapix-mcp.exe','LICENSE-MIT.txt','LICENSE-APACHE.txt','THIRD-PARTY-NOTICES.txt','LEIA-ME-MSI.txt')) {
  $actual=Join-Path $dir $name
  if(-not(Test-Path -LiteralPath $actual)) { throw "Arquivo não instalado: $name" }
  $source=Join-Path $env:GITHUB_WORKSPACE "target/msi-payload/$name"
  if((Get-FileHash -LiteralPath $actual).Hash -ne (Get-FileHash -LiteralPath $source).Hash) { throw "Hash instalado divergente: $name" }
}
$link=Join-Path $env:APPDATA 'Microsoft/Windows/Start Menu/Programs/Etapix MSI/Etapix.lnk'
if(-not(Test-Path -LiteralPath $link)) { throw 'Atalho ausente' }
$shell=New-Object -ComObject WScript.Shell
if($shell.CreateShortcut($link).TargetPath -ne (Join-Path $dir 'etapix.exe')) { throw 'Atalho aponta para destino incorreto' }
$sentinel=Join-Path $dir 'projeto-usuario-nao-apagar.txt'
[IO.File]::WriteAllText($sentinel,'preservar dados desconhecidos')
Remove-Item -LiteralPath (Join-Path $dir 'etapix.exe')
Run-Msi @('/famus',"`"$Package`"",'/qn','/norestart','/L*v',"`"$logDir/repair.log`"")
if(-not(Test-Path -LiteralPath (Join-Path $dir 'etapix.exe'))) { throw 'Reparo não restaurou o executável' }
Run-Msi @('/i',"`"$UpgradePackage`"",'/qn','/norestart','/L*v',"`"$logDir/upgrade.log`"")
if(-not(Test-Path -LiteralPath $sentinel)) { throw 'Upgrade apagou dados desconhecidos' }
Run-Msi @('/x',$upgradeProduct,'/qn','/norestart','/L*v',"`"$logDir/uninstall.log`"")
foreach($name in @('etapix.exe','etapix-mcp.exe','LICENSE-MIT.txt','LICENSE-APACHE.txt','THIRD-PARTY-NOTICES.txt','LEIA-ME-MSI.txt')) {
  if(Test-Path -LiteralPath (Join-Path $dir $name)) { throw "Arquivo do produto permaneceu: $name" }
}
if(Test-Path -LiteralPath $link) { throw 'Atalho permaneceu' }
if(-not(Test-Path -LiteralPath $sentinel)) { throw 'Remoção apagou dados desconhecidos' }
foreach($file in $clients) { $after=if(Test-Path -LiteralPath $file) {(Get-FileHash -LiteralPath $file).Hash} else {'ABSENT'}; if($after -ne $before[$file]) { throw "Configuração do cliente alterada: $file" } }
'PASS: install, hashes, shortcut, repair, upgrade, uninstall, preserve-user-data, preserve-client-config, no-custom-actions' | Set-Content (Join-Path $logDir 'result.txt')
