#!/bin/sh
# Instala o Etapix para o usuário atual (sem sudo): programa em ~/.local/share/etapix, atalho no
# menu de aplicativos e o conector para o Claude Code e o Codex, se estiverem instalados.
set -e
here="$(cd "$(dirname "$0")" && pwd)"
dir="$HOME/.local/share/etapix"
apps="$HOME/.local/share/applications"
icons="$HOME/.local/share/icons/hicolor/256x256/apps"

mkdir -p "$dir" "$apps" "$icons" "$HOME/.local/bin"
install -m 755 "$here/etapix" "$here/etapix-mcp" "$dir/"
install -m 755 "$here/desinstalar.sh" "$dir/"
install -m 644 "$here/etapix.png" "$icons/etapix.png"
ln -sf "$dir/etapix" "$HOME/.local/bin/etapix"

cat > "$apps/etapix.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Etapix
Comment=Planejador de projetos compatível com o Microsoft Project
Exec="$dir/etapix" %f
Icon=etapix
Terminal=false
Categories=Office;ProjectManagement;
MimeType=application/xml;
EOF
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps" >/dev/null 2>&1 || true

# The connector registers itself in Claude Code (~/.claude.json) and Codex (~/.codex/config.toml).
"$dir/etapix-mcp" instalar

echo
echo "Etapix instalado. Abra pelo menu de aplicativos ou com o comando: etapix"
