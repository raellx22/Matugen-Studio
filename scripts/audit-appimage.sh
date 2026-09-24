#!/usr/bin/env bash
set -euo pipefail

appimage="$(realpath "${1:?usage: audit-appimage.sh AppImage}")"
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
file "$appimage"
sha256sum "$appimage"
workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT
cd "$workdir"
chmod +x "$appimage"
"$appimage" --appimage-extract >/dev/null

appdir="$workdir/squashfs-root"
test -d "$appdir"
desktop_file="$(find "$appdir" -type f -name '*.desktop' -print -quit)"
test -n "$desktop_file"
grep -q '^Icon=' "$desktop_file"
test -n "$(find "$appdir" -type f -name 'catalog.json' -print -quit)"
test -n "$(find "$appdir" -type f -name 'discord-material.css' -print -quit)"
test -n "$(find "$appdir" -type f -name 'tray.png' -print -quit)"
test -n "$(find "$appdir" -type f -name '*.png' -print -quit)"
test -f "$appdir/usr/share/metainfo/io.github.raellx22.matugenstudio.metainfo.xml"
cmp "$project_root/src-tauri/icons/32x32.png" \
  "$appdir/usr/share/icons/hicolor/32x32/apps/matugen-studio.png"
cmp "$project_root/src-tauri/icons/tray.png" \
  "$appdir/usr/lib/Matugen Studio/icons/tray.png"
grep -a -q '/matugen-studio.png' "$appdir/usr/bin/matugen-studio"
for library in \
  libwayland-client.so.0 libwayland-cursor.so.0 \
  libwayland-egl.so.1 libwayland-server.so.0 \
  libxkbcommon.so.0 libxcb-randr.so.0 \
  libxcb-render.so.0 libxcb-shm.so.0 \
  libXau.so.6 libXdmcp.so.6; do
  test ! -e "$appdir/usr/lib/$library"
done

echo "Desktop entry: $desktop_file"
grep '^Icon=' "$desktop_file"
echo 'Highest GLIBC symbol requirement in ELF files:'
find "$appdir" -type f -print0 | xargs -0 -r file | awk -F: '/ELF/ {print $1}' | while IFS= read -r elf; do
  readelf --version-info "$elf" 2>/dev/null | grep -o 'GLIBC_[0-9][0-9.]*' || :
done | sort -Vu | tail -1

echo 'ELF files with unresolved linked libraries on the CI host (review with AppRun library paths):'
find "$appdir" -type f -print0 | xargs -0 -r file | awk -F: '/ELF.*executable/ {print $1}' | while IFS= read -r elf; do
  ldd "$elf" 2>/dev/null | grep 'not found' || :
done
