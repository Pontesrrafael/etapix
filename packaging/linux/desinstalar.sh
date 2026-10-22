#!/bin/sh
# Remove o Etapix do usuário atual (seus arquivos de projeto não são apagados).
dir="$HOME/.local/share/etapix"
[ -x "$dir/etapix-mcp" ] && "$dir/etapix-mcp" remover
rm -f "$HOME/.local/share/applications/etapix.desktop" \
      "$HOME/.local/share/icons/hicolor/256x256/apps/etapix.png" \
      "$HOME/.local/bin/etapix"
rm -rf "$dir"
echo "Etapix removido."
