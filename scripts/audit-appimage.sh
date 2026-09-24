#!/usr/bin/env bash
set -euo pipefail

appimage="$(realpath "${1:?usage: audit-appimage.sh AppImage}")"
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
