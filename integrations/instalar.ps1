# Installs the Etapix connector (MCP server) and skill for the current user, in every assistant
# found on this machine: Claude Code, Codex and Claude Desktop. Run through instalar.cmd.
# (The Etapix installer does the same on its own; this is for installing only the connector.)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$dir = Join-Path $env:LOCALAPPDATA 'Programs\etapix'
$exe = Join-Path $dir 'etapix-mcp.exe'

Write-Host 'Etapix para Claude e Codex' -ForegroundColor Green
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item (Join-Path $here 'etapix-mcp.exe') $exe -Force
Copy-Item (Join-Path $here 'etapix.mcpb') (Join-Path $dir 'etapix.mcpb') -Force
Write-Host "  Conector instalado em $exe"
# The connector registers itself (Claude Code, Codex) and opens the Claude Desktop extension.
& $exe instalar (Join-Path $dir 'etapix.mcpb')

Write-Host ''
Write-Host 'Pronto. Reinicie o Claude ou o Codex e peça, por exemplo:' -ForegroundColor Green
Write-Host '  "Crie um cronograma de implantação começando em 02/11/2026 em C:\Projetos\implantacao.mpp"'
Write-Host ''
Write-Host 'No claude.ai (navegador ou app), a skill se instala em Configurações > Capacidades >'
Write-Host 'Skills > Enviar skill, escolhendo o arquivo etapix-skill.zip desta pasta.'
