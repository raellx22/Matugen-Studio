#!/usr/bin/env bash
set -euo pipefail

input="$(realpath "${1:?usage: prepare-appimage.sh input.AppImage [output.AppImage]}")"
output="$(realpath -m "${2:-$input}")"
test -f "$input"

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT
cd "$workdir"
"$input" --appimage-extract >/dev/null
appdir="$workdir/squashfs-root"
libdir="$appdir/usr/lib"

# linuxdeploy bundles Ubuntu display-client libraries that conflict with newer
# host Mesa/EGL on Wayland. Keep these host-side, as GPU/display libraries are.
display_libraries=(
  libwayland-client.so.0 libwayland-cursor.so.0
  libwayland-egl.so.1 libwayland-server.so.0
  libxkbcommon.so.0 libxcb-randr.so.0
  libxcb-render.so.0 libxcb-shm.so.0
  libXau.so.6 libXdmcp.so.6
)
for library in "${display_libraries[@]}"; do
  test -f "$libdir/$library" || { echo "Expected bundled library missing: $library" >&2; exit 1; }
  rm -- "$libdir/$library"
done

offset="$("$input" --appimage-offset)"
[[ "$offset" =~ ^[0-9]+$ ]] && test "$offset" -gt 0
head -c "$offset" "$input" > "$workdir/runtime-x86_64"

tool="$workdir/appimagetool-x86_64.AppImage"
if [[ -n "${APPIMAGETOOL_PATH:-}" ]]; then
  cp -- "$APPIMAGETOOL_PATH" "$tool"
else
  curl -fsSL --retry 3 \
    https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage \
    -o "$tool"
fi
printf '%s  %s\n' \
  ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0 \
  "$tool" | sha256sum --check -
chmod +x "$tool"

ARCH=x86_64 "$tool" --appimage-extract-and-run \
  --runtime-file "$workdir/runtime-x86_64" \
  "$appdir" "$workdir/patched.AppImage"
test -s "$workdir/patched.AppImage"
"$workdir/patched.AppImage" --appimage-offset >/dev/null
mkdir -p "$(dirname "$output")"
mv -- "$workdir/patched.AppImage" "$output"
echo "Prepared AppImage with host display libraries: $output"
