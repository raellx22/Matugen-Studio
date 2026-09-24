# Curated template catalog

Matugen Studio groups templates as **application → visual variant → installation target**. A target is a specific output path and client/install type, not a second application card. For example, Discord has Midnight, System24, and Material You variants; Material You can target Vesktop or Equibop, Native or Flatpak, without duplicating the Discord card. Its Equibop outputs use `matugen-material.css`, separate from Midnight and System24.

## Sources and distribution

- **Official** means the bundled template comes from the MIT-licensed [matugen-themes](https://github.com/InioX/matugen-themes) collection. Its license and attribution remain in `THIRD_PARTY_NOTICES.md` and the resource `LICENSE`.
- **Community** means a reviewed, separately attributed contribution, not an automatic mirror of another repository. The bundled Discord Material You adapter and VS Code Material Premium color fragment are original Matugen Studio code, licensed under this project's GPL-2.0-or-later license. Discord uses the [documented Material Discord variable interface](https://github.com/CapnKitten/BetterDiscord/blob/master/Themes/Material-Discord/README.md), with inspiration from [Noctalia community templates](https://github.com/noctalia-dev/community-templates/tree/main/discord); it does **not** bundle Noctalia's CSS or CapnKitten's CSS. The VS Code fragment was designed independently after auditing [Zatch07's installation approach](https://github.com/Zatch07/matugen-themes/tree/main/vscode); none of that unlicensed settings file is copied.
- CapnKitten's base Material Discord and Material You add-on are imported over HTTPS when the Discord client loads the generated adapter. Their upstream repositories do not advertise a redistribution license. Studio does not download or vendor them. First load needs a working network connection; a client cache might help later but is not guaranteed. The imports are upstream-controlled and can change or disappear. The user must explicitly select the installed CSS theme in their Discord client.
- VS Code's variant writes a standalone JSON file under the XDG cache directory, not `User/settings.json`. Its only keys are `workbench.colorCustomizations` and `editor.tokenColorCustomizations`. To use it, open **Preferences: Open User Settings (JSON)** in VS Code and merge the two generated objects into your own settings, preserving every other key. This is deliberately a manual step: Matugen Studio will not overwrite font, autosave, icon theme, trust or startup preferences. Refreshing the fragment does not automatically merge it into VS Code.
- A remote registry is not implemented. Source information can hold repository, source path, version and upstream commit for later provenance tracking; these fields do not imply update or execution rights.

Missing or ambiguous licensing is a **stop** for bundling someone else's template. A repository README showing how to use a template does not grant redistribution. Review the license for the specific file and any embedded fonts, screenshots, downloaded CSS or scripts. If no grant exists, seek permission or write an independently authored integration from public interface documentation; do not route around a missing grant with automatic downloads.

## Contributor workflow

1. Inspect the upstream template, manifest, README, license, output paths, dependencies and *every* hook/script. Document native/Flatpak and dark/light behavior. Reject arbitrary download/exec, sudo, destructive deletion and profile-wide edits.
2. Put the renderable source under `src-tauri/resources/matugen-themes/templates/`. It must use Matugen Studio's existing parser and context (`{{ colors.primary.default.hex }}` and `.dark`/`.light` for explicit mode roles). Never commit a rendered personal config or an unlicensed upstream source file.
3. Add one application ID, one stable variant ID per visual style, and one target ID per installation destination in `src-tauri/resources/matugen-themes/catalog.json`. Give every target an input file, a non-empty output path, a native/Flatpak/manual type and a detection directory if reliable. Use `$XDG_CONFIG_HOME`/`$XDG_DATA_HOME`/`$XDG_STATE_HOME` or `~`, never a developer's home directory. Distinct variants should use distinct output filenames when the app supports multiple themes.

4. Include the variant's author, source kind, source repository, source path where applicable, license and attribution. If external CSS is imported at runtime, label its network/offline limitation and do not claim it is bundled or licensed under Studio's license.
5. Use a dedicated theme file/import wherever possible. For an existing config that must be patched, back up the original, provide an idempotent reversible change, and require explicit user action. Do not add an unaudited `post_hook` or silently replace an entire user settings/profile file.
6. Render each candidate with a real Matugen palette in both modes; reject raw `{{`/`}}`. Parse JSON/TOML/XML as appropriate, validate CSS/INI structure, and exercise installation and selective removal with isolated XDG test directories. Include upstream/version assumptions and any one-time setup in the variant details. Add a third-party notice for licensed third-party material actually bundled.

For a new community entry, the relevant shape inside `catalog.json` is:

```json
{
  "id": "example",
  "name": "Example",
  "category": "Productivity",
  "variants": [{
    "id": "example.material",
    "name": "Material",
    "sourcePath": "example-material.css",
    "description": "What this changes and which manual step it needs.",
    "source": {
      "kind": "community",
      "name": "Example contributors",
      "repository": "https://example.org/example",
      "author": "Example contributors",
      "license": "MIT",
      "licenseStatus": "repository-declared",
      "attribution": "Source and authorship verified for this exact file."
    },
    "targets": [{
      "id": "native",
      "label": "Native",
      "installType": "native",
      "outputPath": "$XDG_CONFIG_HOME/example/themes/matugen-material.css",
      "detectPath": "$XDG_CONFIG_HOME/example"
    }]
  }]
}
```

The example URL is illustrative, not an approved source. `sourcePath` is a
filename relative to the bundled `templates/` directory; a target can override
it when its native/Flatpak input differs. Omit `detectPath` when detection is
unreliable; the target remains manually selectable. An official entry can omit
`source` to inherit the bundled matugen-themes MIT provenance. The catalog
parser still discovers older files not listed here and groups them by target
application, so only new curated variants need explicit metadata.

The existing template-file discovery and old `install_template`, `preview_template`, `uninstall_template` commands stay available for older presets/configs. Catalog metadata layers over those files; adding a new variant should not require a new Rust filename switch. Installed state is per variant **and target**, not per application.

## First-batch curation

`NEEDS WORK` means **not bundled**. Every upstream community candidate below
also needs an explicit redistribution grant (or an independently authored
replacement) before inclusion; functional feasibility does not override that
gate. `REJECTED FOR NOW` likewise means no bundled upstream source.

| Candidate | Decision | Reason |
| --- | --- | --- |
| Discord Material You | READY as an independently authored adapter | Original Matugen role mapping; external base/add-on imports require network, and no upstream CSS is bundled. |
| LibreOffice | NEEDS WORK | `.oxt` extension packaging, `unopkg` remove/add, application-shutdown requirement and one-time scheme selection; never copy the hook blindly. |
| Blender | NEEDS WORK | Native/Flatpak hooks execute Python in headless Blender and persist user preferences; requires version checks, a backup and reversible application. |
| GIMP | NEEDS WORK | One-file CSS targets GIMP 3.2 native/Flatpak only and requires the Default base theme; verify version and licensing before enabling. |
| Inkscape | NEEDS WORK | Native/Flatpak `user.css` requires the Minwaita-Inkscape base theme; it cannot recolor the canvas desk. |
| Darktable | NEEDS WORK | Native and Flatpak CSS import different base-theme paths; requires separate inputs, manual theme selection and version checks. |
| Fastfetch | NEEDS WORK | `jq` merges generated colors into `config.jsonc`, but upstream hook rejects comments/trailing commas; preserve existing configs and validate a safe merge. |
| Brave | REJECTED FOR NOW | Hook generates PNG assets and an unpacked extension, optionally mutates profile `Preferences` after backup; `.rgb_csv` is unsupported locally. Require explicit advanced/manual setup. |
| Antigravity | NEEDS WORK | Noctalia uses a `jq` config merge; Zatch07 writes an entire VS Code-fork `settings.json` and needs an extension. Prefer a safe merge after version/license review. |
| VS Code Material Premium | READY as an independently authored color-only fragment | Original Material role mapping, with only `workbench.colorCustomizations` and `editor.tokenColorCustomizations`; explicit manual merge, never overwrites `settings.json`. Zatch07's upstream personal settings and extension are not bundled. |
| Spotify community | REJECTED FOR NOW as incompatible candidate | Community mapping targets Spicetify wal and uses a Hyprland hook; existing official Sleek `[matugen]` mapping is a different theme and already supported. Not interchangeable without a separate safe wal installation contract. |
| Konsole | REJECTED FOR NOW | Existing KDE integration is preferable; Noctalia hook writes active PTYs, and `rgb_csv`/terminal roles are unsupported locally. |

No Noctalia, Zatch07 or CapnKitten source files are bundled: none of those repositories published a confirmed root redistribution license at review time. These decisions are about the current integration and rights, not a judgment of upstream quality.
