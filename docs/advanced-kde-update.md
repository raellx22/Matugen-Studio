# Advanced Material / KDE update audit

Baseline: `origin/main` at `6f7f456`. Vendored matugen-core 4.1.0 and
material-colors 0.4.2 (Cargo requirement 0.4.0). Prowl was unavailable.

## Engine decision

Matugen upstream 4.2.0 (`ca94b8b`) still depends on material-colors 0.4.0.
Updating all of Matugen would also replace its template parser and is unnecessary.
The sole core change extracts its existing quantization/ranking into a public,
noninteractive candidate function. CLI selection keeps the same implementation.

Released material-colors 0.4.2 has no specification selector. Its upstream
0.5.0 development revision `3fe7f52cde8c221d9a44dec84bca3706a71bb080` implements
2025 roles and palettes, but changes Argb to Rgb and contains unfinished 2021
palette fallback helpers and 2026 branches. We keep 0.4.2 for 2021 and the whole
vendored core. A dependency alias pinned to that immutable revision is used
only by the 2025 renderer. No upstream implementation is copied into Studio.
2026 is neither accepted nor exposed. The adapter uses existing variant
constructors for Content, Fidelity, Fruit Salad, Rainbow and Monochrome, avoiding
unfinished fallback helpers; Neutral, Tonal Spot, Expressive and Vibrant use the
2025 palette constructors. All nine variants are tested in both modes at three
contrast levels. 2021 with default controls is compared exactly to the old engine.

2025 is the new selection default. Stored preset color snapshots remain unchanged;
regenerating an old preset with absent fields uses the new defaults.

Contrast uses MCU's [-1, 1], default 0. Chroma is an HCT multiplier [0, 10],
default 1. Tone multiplies background-role lightness [0, 1.5], default 1;
light mode clamps the multiplier to at least 0.5. The multiplier policy follows
KDE Material You Colors, not a claimed intrinsic MCU multiplier range. HCT solves
sRGB gamut limits and clamps resulting tone to [0, 100]. Strong adjustments can
reduce contrast; defaults bypass adjustment and preserve engine output exactly.

## Persistence and integrations

`$XDG_CONFIG_HOME/matugen-studio/settings.json` owns generation settings,
KDE integrations, scheme, mode, GTK preference and KDE overrides. Atomic writes
and a lock protect application writes. Existing localStorage GTK/override values
are migrated once. The watcher reads the current settings for every wallpaper,
including when the main window is hidden. Service enable/background/poll settings
stay in the existing `matugen/service.json`.

Presets keep version 2. New generation fields are flattened into `scheme`, reusing
its original contrast key (legacy null means zero). `desktop.integrations` defaults
to disabled. Existing color snapshots, assets, templates and overrides are retained.
Invalid spec strings and nonfinite/out-of-range numeric settings are rejected.
Missing or invalid seed indices fall back to candidate zero without inventing colors.

Klassy detection queries `klassy-settings --version`. Supported versions are 6.5+
within major 6, whose current keys are under `klassy/klassyrc`. Only the active
custom outline and optional active titlebar opacity/override are changed. 6.7.3
was detected on this machine; the initial active decoration was Breeze.

Rounded Corners detection asks KWin for the actual available effect ID
`kwin4_effect_shapecorners`. Unknown effect IDs/forks are not guessed. Only outline
colors and custom-color selection are patched in `[Round-Corners]` in `kwinrc`.
Radius, shadow, geometry, thickness, alpha and gradient settings are preserved.
Automatic maps to Material outline, with Primary/Outline/Outline Variant choices.

First writes keep a `.matugen-backup` copy. Writes preserve unrelated INI keys,
comments and file permissions; immutable KConfig groups/keys are rejected.
Identical content is not rewritten or reloaded. Klassy receives color-cache and
reloadConfig D-Bus signals. Only the loaded Rounded Corners effect is reconfigured.
No process restarts or KWin termination. Disabling an integration stops future
writes; it does not undo previously applied colors.

## Phase 1 checkpoint

27 application unit tests passed, including real multicolor-image extraction,
2021 baseline equality, both specifications/all variants, numeric bounds, preset
legacy migration and actual import/export in an isolated child test process,
INI preservation/idempotence, detection parsing and absent/disabled integrations.
Core tests, npm ci, npm run build, cargo fmt --check, cargo clippy, cargo test and
cargo build passed. Baseline formatting inconsistencies in the Tauri crate were
normalized by cargo fmt. Existing core/bridge Clippy warnings remain; no lint
suppression was added. npm ci reported 7 existing dependency advisories.

## References

- [Matugen](https://github.com/InioX/matugen/tree/ca94b8b)
- [Material colors pinned source](https://github.com/Aiving/material-colors/tree/3fe7f52cde8c221d9a44dec84bca3706a71bb080)
- [KDE Material You Colors](https://github.com/luisbocanegra/kde-material-you-colors)
- [Klassy](https://github.com/paulmcauley/klassy)
- [Rounded Corners](https://github.com/matinlotfali/KDE-Rounded-Corners)

## Phase 2 desktop validation (KDE Plasma 6 Wayland)

The actual `npm run tauri dev` window was exercised through WebKit's remote
inspector and visually captured with Spectacle. No mocked frontend or permanent
test harness was introduced. With a multicolor local wallpaper, seed 1 changed
Source Color from `#0ACBFE` to `#1DF8CC`, the KDE selection color and the rendered
Kitty template without changing the wallpaper. The preview initially failed to
load gallery paths; successful image generation now grants that file to Tauri's
asset protocol. Preset images receive the same permission when loaded after an
app restart; this was reproduced as a missing preview, fixed, and verified.

Contrast -0.5/0/0.5, chroma 0.5/1/2, tone 0.75/1/1.25, 2021/2025 and
light/dark modes changed generated/applied KDE colors without invalid hex or a
crash. The 2021/2025 palettes differed on the same image and seed. Light-mode
control contrast was improved by inheriting the window color scheme. Strong
chroma/tone combinations can still produce near-white accent colors; the HCT
engine constrains gamut, while users retain responsibility for perceptual
contrast outside the engine's contrast parameter.

Klassy 6.7.3 and the loaded `kwin4_effect_shapecorners` effect were detected.
Active Klassy outline followed Material primary; managed titlebar opacity applied
80%. Rounded Corners followed the selected outline role. KWin's PID stayed the
same and corner geometry was not touched. Two external wallpaper changes were
picked up by the backend watcher with seed/contrast/chroma/tone/spec settings;
KDE, GTK, installed templates and decoration outlines updated. On the second
image, the requested seed 2 did not exist, so the saved/effective seed became 0.
The actual KDE selection RGB matched direct generation for that image.

A v2 preset containing generation settings, mode and integration settings was
saved through the UI, and a v1 `.matugen` fixture was imported; missing advanced
fields defaulted correctly. Both remained visible after app restart. The v2
preset restored its palette and wallpaper preview. Local gallery pagination,
favorites, two-monitor targeting, manual KDE color edits, template overrides,
Wallhaven search (24 results) and the system tray were also exercised. These are
bounded checks of representative flows, not an exhaustive pass over every
bundled template or monitor arrangement.
