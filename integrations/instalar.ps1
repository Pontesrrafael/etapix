# Installs the Etapix connector (MCP server) and skill for the current user, in every assistant
# found on this machine: Claude Code, Codex and Claude Desktop. Run through instalar.cmd.
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$dir = Join-Path $env:LOCALAPPDATA 'Programs\etapix'
$exe = Join-Path $dir 'etapix-mcp.exe'

Write-Host 'Etapix para Claude e Codex' -ForegroundColor Green
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item (Join-Path $here 'etapix-mcp.exe') $exe -Force
Write-Host "  Conector instalado em $exe"

function Install-Skill($root, $who) {
    $target = Join-Path $root 'skills\etapix'
    New-Item -ItemType Directory -Force $target | Out-Null
    Copy-Item (Join-Path $here 'skill\etapix\SKILL.md') $target -Force
    Write-Host "  Skill instalada para o $who"
}

# Claude Code: MCP server (user scope) and skill.
$claude = Get-Command claude -ErrorAction SilentlyContinue
if ($claude -or (Test-Path (Join-Path $env:USERPROFILE '.claude'))) {
    Install-Skill (Join-Path $env:USERPROFILE '.claude') 'Claude Code'
    if ($claude) {
        & claude mcp remove --scope user etapix 2>$null | Out-Null
        & claude mcp add --scope user etapix -- $exe | Out-Null
        Write-Host '  Conector registrado no Claude Code'
    } else {
        # Claude Code inside the desktop app has no 'claude' command: write the user-scope entry
        # in ~/.claude.json directly (a copy of the old file is kept next to it).
        $json = Join-Path $env:USERPROFILE '.claude.json'
        $data = if (Test-Path $json) { Get-Content $json -Raw -Encoding UTF8 | ConvertFrom-Json } else { New-Object psobject }
        if (Test-Path $json) { Copy-Item $json "$json.bak-etapix" -Force }
        if (-not $data.mcpServers) { $data | Add-Member -NotePropertyName mcpServers -NotePropertyValue (New-Object psobject) -Force }
        $entry = [pscustomobject]@{ type = 'stdio'; command = $exe; args = @(); env = (New-Object psobject) }
        $data.mcpServers | Add-Member -NotePropertyName etapix -NotePropertyValue $entry -Force
        [IO.File]::WriteAllText($json, ($data | ConvertTo-Json -Depth 100), (New-Object Text.UTF8Encoding $false))
        Write-Host '  Conector registrado no Claude Code'
    }
}

# Codex: [mcp_servers.etapix] in ~/.codex/config.toml, and the skill.
$codex = Join-Path $env:USERPROFILE '.codex'
if ((Get-Command codex -ErrorAction SilentlyContinue) -or (Test-Path $codex)) {
    New-Item -ItemType Directory -Force $codex | Out-Null
    $config = Join-Path $codex 'config.toml'
    $text = if (Test-Path $config) { Get-Content $config -Raw } else { '' }
    if ($text -notmatch '\[mcp_servers\.etapix\]') {
        $entry = "`r`n[mcp_servers.etapix]`r`ncommand = '$exe'`r`nargs = []`r`n"
        [IO.File]::AppendAllText($config, $entry, (New-Object Text.UTF8Encoding $false))
    }
    Write-Host '  Conector registrado no Codex'
    Install-Skill $codex 'Codex'
}

# Claude Desktop: opening the .mcpb shows its install dialog (one click on Instalar).
$mcpb = Join-Path $here 'etapix.mcpb'
if (Test-Path (Join-Path $env:APPDATA 'Claude')) {
    Write-Host '  Abrindo o Claude Desktop para instalar o conector: clique em Instalar.'
    Start-Process $mcpb
}

Write-Host ''
Write-Host 'Pronto. Reinicie o Claude ou o Codex e peça, por exemplo:' -ForegroundColor Green
Write-Host '  "Crie um cronograma de implantação começando em 02/11/2026 em C:\Projetos\implantacao.mpp"'
Write-Host ''
Write-Host 'No claude.ai (navegador ou app), a skill se instala em Configurações > Capacidades >'
Write-Host 'Skills > Enviar skill, escolhendo o arquivo etapix-skill.zip desta pasta.'
