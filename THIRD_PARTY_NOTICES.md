# Third-Party Notices

Matugen Studio bundles and/or derives work from the following projects:

## matugen-core

- Source: https://github.com/InioX/matugen
- License: GPL-2.0-or-later
- Local copy: `matugen-core/`
- License text: `matugen-core/LICENSE`

## matugen-themes

- Source: https://github.com/InioX/matugen-themes
- License: MIT
- Bundled path: `src-tauri/resources/matugen-themes/`
- License text: `src-tauri/resources/matugen-themes/LICENSE`

## adw-gtk3

- Source: https://github.com/lassekongo83/adw-gtk3
- License: LGPL-2.1
- Bundled path: `src-tauri/resources/gtk-themes/`
- Notice: `src-tauri/resources/gtk-themes/NOTICE.md`
- License text: `src-tauri/resources/gtk-themes/LGPL-2.1-only.txt`

Matugen Studio rewrites color definitions in derived GTK theme copies generated
from the bundled adw-gtk3 assets.

## Material colors (2025 adapter)

The 2025 renderer links `material-colors` from Aiving's upstream repository at
revision `3fe7f52cde8c221d9a44dec84bca3706a71bb080`, alongside the original 0.4.2
library used by matugen-core. Licensed under MIT OR Apache-2.0. Source and license
texts: https://github.com/Aiving/material-colors/tree/3fe7f52cde8c221d9a44dec84bca3706a71bb080.

## Discord Material You adapter

- Bundled path: `src-tauri/resources/matugen-themes/templates/discord-material.css`
- Author: Matugen Studio contributors
- License: GPL-2.0-or-later (original adapter; see project `LICENSE`)
- Inspiration: https://github.com/noctalia-dev/community-templates/tree/main/discord
- External runtime themes: https://github.com/CapnKitten/BetterDiscord/tree/master/Themes/Material-Discord

No Noctalia or CapnKitten stylesheet is bundled. The adapter imports Material
Discord and its Material You add-on from CapnKitten's published URLs when the
Discord client loads the generated CSS. Those external files have no confirmed
redistribution license in their repositories; they are not included in this app.
Availability and changes to those online stylesheets are outside Matugen Studio's
control. The adapter maps Matugen's own color roles to the public theme-variable
interface documented by Material Discord.

## VS Code Material Premium color fragment

- Bundled path: `src-tauri/resources/matugen-themes/templates/vscode-material-premium.json`
- Author: Matugen Studio contributors
- License: GPL-2.0-or-later (original color mapping; see project `LICENSE`)
- Audited upstream approach: https://github.com/Zatch07/matugen-themes/tree/main/vscode

The upstream settings file is not bundled or copied. The generated fragment
contains theme-only JSON keys and must be merged into VS Code manually.
