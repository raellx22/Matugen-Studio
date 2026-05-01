# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Matugen Studio is a Tauri 2 desktop application that wraps the `matugen-core` library in a GUI. It generates Material You color schemes from images/colors and applies them to Linux desktop applications via a template system.

## Commands

### Development
```bash
npm run tauri dev         # Run full app with Tauri (use this for dev)
npm run dev               # Vite-only dev server (frontend without Tauri IPC)
```

The `tauri` script sets `WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1` for Linux GPU compatibility.

### Build
```bash
npm run build             # TypeScript check + Vite frontend build
npm run tauri build       # Full app binary
```

### Rust (backend + core library)
```bash
cd src-tauri && cargo build
cd matugen-core && cargo build
cd matugen-core && cargo test
```

## Architecture

The project is a three-layer monorepo:

### 1. React Frontend (`src/`)
- `App.tsx` — Main UI (~2000 lines): tab navigation, color wheel picker, scheme type selector. Holds the primary application state and orchestrates IPC calls.
- `pages/Source.tsx` — Wallpaper browser with multi-monitor support and thumbnail generation.
- `pages/Templates.tsx` — Template browser, installer, previewer for 30+ app themes (Alacritty, Hyprland, GTK, Waybar, Discord, Zed, etc.).
- `pages/Presets.tsx` — Save/load/export/import color scheme presets.
- `pages/KdeIntegration.tsx` — KDE Plasma wallpaper sync and color scheme management.
- `utils/templateDefaults.ts` — Output path mappings for all supported app templates.

### 2. Tauri Bridge (`src-tauri/src/commands/`)
Thin Rust layer that exposes `matugen-core` functionality as Tauri IPC commands:
- `color.rs` — `generate_scheme_from_image(imagePath, schemeType)` → Material You palette
- `template.rs` — `list_available_templates`, `preview_template`, `apply_theme`, `install_template`
- `desktop.rs` — `list_wallpapers`, `apply_wallpaper`, thumbnail generation, monitor detection
- `kde.rs` — `generate_kde_colorscheme_cmd`, `apply_kde_colorscheme`
- `preset.rs` — `save_preset`, `get_presets`, `delete_preset`, `export_preset`, `import_preset`

All commands are registered in `lib.rs`.

### 3. Core Library (`matugen-core/`)
A vendored/forked copy of [InioX/matugen](https://github.com/InioX/matugen). Key subsystems:
- `color/` — Material You color extraction (via `material-colors` crate), Base16 palette, format converters (hex/rgb/hsl/hsv)
- `parser/` — Custom template DSL built with Chumsky: supports piping, filters, conditionals, loops, includes, and JSON context injection
- `filters/` — Color transforms (`set_lightness`, `set_saturation`, `rotate_hue`) and string ops (`replace`, `to_upper`, `to_lower`)
- `scheme.rs` — 8 Material Design scheme types: Content, Expressive, Fidelity, Fruit Salad, Monochrome, Neutral, Rainbow, Tonal Spot, Vibrant
- `cache.rs` — SHA256-based image cache

## Key Design Decisions

- The Tauri `asset` protocol is enabled to allow the frontend to display local images (wallpapers, thumbnails) without copying them.
- `matugen-core` is a workspace dependency (`path = "../matugen-core"`) — changes to the library are picked up immediately by the Tauri backend.
- Template files use a custom DSL (not Handlebars/Jinja) — see `matugen-core/src/parser/` for syntax.
- The app targets Linux desktop environments (GNOME, KDE, Hyprland, Sway); KDE has its own dedicated integration tab.
