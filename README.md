# Matugen Studio

Matugen Studio is a Tauri 2 desktop app for generating Material You palettes
from wallpapers and applying them to Linux desktop themes.

The first release focuses on KDE Plasma, with support for:

- wallpaper-driven color generation through `matugen-core`
- KDE Plasma color schemes
- GTK theme generation based on adw-gtk3
- bundled Matugen templates for supported apps
- a KDE wallpaper watch service for automatic recoloring

## Build

```bash
npm ci
npm run tauri build -- --bundles appimage
```

On rolling distributions where `linuxdeploy` fails while stripping libraries,
use:

```bash
NO_STRIP=1 npm run tauri build -- --bundles appimage
```

## License

Matugen Studio is licensed under GPL-2.0-or-later. See [LICENSE](LICENSE).

Bundled third-party assets keep their original licenses. See
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
