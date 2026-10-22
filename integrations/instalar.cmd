@echo off
rem Instala o conector e a skill do Etapix no Claude Code, no Codex e no Claude Desktop.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0instalar.ps1"
echo.
pause
