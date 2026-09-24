# Matugen Studio 2.0 RC1

This prerelease is for Linux x86_64 cross-distribution testing. The AppImage is built on Ubuntu 22.04.

## New Identity

- New MS application logo, desktop and window icons, favicon, and in-app branding.
- Simplified MS system tray icon.

## New UI

- Redesigned interface with a consistent visual system, responsive layouts, improved dropdowns and dialogs, and refined motion.

## Material You

- Seed selection, Material 2021/2025 generation, and contrast, chroma, and tone controls.

## KDE Integration

- Optional Klassy and KDE Rounded Corners integrations, with improved KDE color controls.

## Wallpapers

- Multi-source Wallpaper Library, KDE/system wallpaper discovery, and Wallhaven browsing.
- Permanent downloads are separate from temporary cache and managed active wallpapers.

## Templates

- Application → Variant → Target catalog and community template support.
- Discord Material You with a separate Equibop target, alongside Midnight and System24.
- VS Code Material Premium, Heroic Native/Flatpak, Foot, Ghostwriter, and updated templates.

## Reliability

- Native dialogs, safer paths, improved template handling, and regression coverage.

## Known limitations

- Linux x86_64 is the only release architecture for this candidate.
- KDE Plasma and third-party applications are optional host integrations and must be installed separately for their respective features.
- Discord Material CSS imports external stylesheets and needs network access on first use.

The AppImage bundles the application and distributable runtime dependencies while relying on standard Linux system components and optional host integrations where appropriate. Fedora KDE, Kubuntu/KDE neon, and openSUSE KDE still need testing.
