use crate::commands::proc;
use matugen_core::parser::Engine;
use matugen_core::util::config::ConfigFile;
use matugen_core::State;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::Duration;
use tauri::Manager;

static TEMPLATE_STORAGE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Upper bound on how long a single template's `post_hook` may run before
/// it is killed. `post_hook`s are best-effort reload commands (some are
/// bundled, some are typed in by the user) — a stuck one must not be able
/// to stall applying colors for every other template behind it.
const POST_HOOK_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInfo {
    name: String,
    display_name: String,
    path: String,
    relative_path: String,
    size: u64,
    category: String,
    target_app: String,
    automation_level: String,
    installable: bool,
    default_output_path: Option<String>,
    default_post_hook: Option<String>,
    required_commands: Vec<String>,
    manual_steps: Vec<String>,
    related_files: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCatalogApp {
    id: String,
    name: String,
    category: String,
    variants: Vec<TemplateCatalogVariant>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCatalogVariant {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    source: TemplateSource,
    targets: Vec<TemplateTarget>,
    template: TemplateInfo,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSource {
    kind: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    license_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    upstream_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attribution: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateTarget {
    id: String,
    label: String,
    install_type: String,
    input_path: String,
    output_path: String,
    detected: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledTemplateEntry {
    key: String,
    input_path: String,
    output_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogMetadata {
    applications: Vec<CatalogMetadataApp>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogMetadataApp {
    id: String,
    name: String,
    category: String,
    variants: Vec<CatalogMetadataVariant>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogMetadataVariant {
    id: String,
    name: String,
    source_path: String,
    description: Option<String>,
    source: Option<CatalogMetadataSource>,
    automation_level: Option<String>,
    manual_steps: Option<Vec<String>>,
    targets: Vec<CatalogMetadataTarget>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogMetadataTarget {
    id: String,
    label: String,
    install_type: String,
    source_path: Option<String>,
    output_path: String,
    detect_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogMetadataSource {
    kind: String,
    name: String,
    repository: Option<String>,
    author: Option<String>,
    license: Option<String>,
    license_status: Option<String>,
    attribution: Option<String>,
    upstream_commit: Option<String>,
    version: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TemplateColorGroup {
    template_name: String,
    display_name: String,
    target_app: String,
    output_path: Option<String>,
    controls: Vec<TemplateColorControl>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TemplateColorControl {
    key: String,
    name: String,
    description: Option<String>,
    source_path: Option<String>,
    original_hex: String,
    current_hex: String,
    overridden: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum TemplateColorOverride {
    Detailed {
        hex: String,
        original_hex: Option<String>,
    },
    Legacy(String),
}

impl TemplateColorOverride {
    fn hex(&self) -> &str {
        match self {
            Self::Detailed { hex, .. } => hex,
            Self::Legacy(hex) => hex,
        }
    }

    fn original_hex(&self) -> Option<&str> {
        match self {
            Self::Detailed { original_hex, .. } => original_hex.as_deref(),
            Self::Legacy(_) => None,
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct TemplateOverrides {
    templates: HashMap<String, HashMap<String, TemplateColorOverride>>,
}

struct ParsedColorControl {
    key: String,
    name: String,
    description: Option<String>,
    source_path: Option<String>,
    original_hex: String,
}

struct ParsedDeclaration {
    key: String,
    value: String,
    description: Option<String>,
}

fn list_templates_from_themes_dir(themes_dir: PathBuf) -> Result<Vec<TemplateInfo>, String> {
    let mut templates = Vec::new();
    let templates_dir = themes_dir.join("templates");

    if !templates_dir.exists() {
        return Err("Templates directory not found".to_string());
    }

    collect_templates(&templates_dir, &templates_dir, &mut templates)?;

    let websites_dir = themes_dir.join("websites");
    if websites_dir.exists() {
        collect_websites(&websites_dir, &mut templates)?;
    }

    templates.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then(a.target_app.cmp(&b.target_app))
            .then(a.relative_path.cmp(&b.relative_path))
    });
    Ok(templates)
}

fn collect_templates(
    root: &Path,
    dir: &Path,
    templates: &mut Vec<TemplateInfo>,
) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            collect_templates(root, &path, templates)?;
            continue;
        }

        if !is_matugen_template(&path) {
            continue;
        }

        let relative_path = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");

        let meta = entry.metadata().map_err(|e| e.to_string())?;
        templates.push(template_info(path, relative_path, meta.len(), true));
    }

    Ok(())
}

fn collect_websites(websites_dir: &Path, templates: &mut Vec<TemplateInfo>) -> Result<(), String> {
    for entry in fs::read_dir(websites_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("css") {
            continue;
        }

        let name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "website.css".to_string());
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        templates.push(template_info(
            path,
            format!("websites/{}", name),
            meta.len(),
            false,
        ));
    }

    Ok(())
}

fn is_matugen_template(path: &Path) -> bool {
    let relative = path.to_string_lossy();
    if relative.contains("/nix-hm-example/")
        || relative.ends_with("/README.md")
        || relative.ends_with("/init.lua")
    {
        return false;
    }

    fs::read_to_string(path)
        .map(|content| content.contains("{{"))
        .unwrap_or(false)
}

fn template_info(
    path: PathBuf,
    relative_path: String,
    size: u64,
    installable: bool,
) -> TemplateInfo {
    let file_name = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| relative_path.clone());
    let meta = template_metadata(&relative_path, &file_name, installable);

    TemplateInfo {
        name: relative_path.clone(),
        display_name: meta.display_name,
        path: path.to_string_lossy().to_string(),
        relative_path,
        size,
        category: meta.category,
        target_app: meta.target_app,
        automation_level: meta.automation_level,
        installable: meta.installable,
        default_output_path: meta.default_output_path,
        default_post_hook: meta.default_post_hook,
        required_commands: meta.required_commands,
        manual_steps: meta.manual_steps,
        related_files: meta.related_files,
    }
}

struct TemplateMetadata {
    display_name: String,
    category: String,
    target_app: String,
    automation_level: String,
    installable: bool,
    default_output_path: Option<String>,
    default_post_hook: Option<String>,
    required_commands: Vec<String>,
    manual_steps: Vec<String>,
    related_files: Vec<String>,
}

fn template_metadata(relative_path: &str, file_name: &str, installable: bool) -> TemplateMetadata {
    let mut meta = TemplateMetadata {
        display_name: friendly_name(file_name),
        category: "Apps".to_string(),
        target_app: target_from_filename(file_name),
        automation_level: "auto".to_string(),
        installable,
        default_output_path: default_output_path(file_name).map(str::to_string),
        default_post_hook: default_post_hook(file_name).map(str::to_string),
        required_commands: vec![],
        manual_steps: manual_steps(file_name),
        related_files: vec![],
    };

    if relative_path.starts_with("websites/") {
        meta.category = "Websites".to_string();
        meta.target_app = "Firefox CSS".to_string();
        meta.automation_level = "manual".to_string();
        meta.installable = false;
        meta.default_output_path = None;
        meta.manual_steps = vec![
            "templates.steps.websiteEnableStylesheets".to_string(),
            "templates.steps.websiteCopyCss".to_string(),
            "templates.steps.websiteImport".to_string(),
        ];
        return meta;
    }

    match file_name {
        // KDE Plasma / Qt / GTK system theming
        "Matugen.colors" => {
            meta.category = "KDE Plasma".to_string();
            meta.display_name = "KDE Color Scheme".to_string();
            meta.target_app = "KDE Plasma".to_string();
            meta.required_commands = vec!["plasma-apply-colorscheme".to_string()];
        }
        "kvantum-colors.kvconfig" | "kvantum-colors.svg" => {
            meta.category = "KDE Plasma".to_string();
            meta.target_app = "Kvantum".to_string();
            meta.automation_level = "config-patch".to_string();
            meta.manual_steps = vec!["templates.steps.kvantumSetTheme".to_string()];
        }
        "qtct-colors.conf" => {
            meta.category = "KDE Plasma".to_string();
            meta.target_app = "Qt qt5ct/qt6ct".to_string();
            meta.automation_level = "config-patch".to_string();
            meta.required_commands = vec!["qt5ct".to_string(), "qt6ct".to_string()];
            meta.manual_steps = vec![
                "templates.steps.qtctPlatformTheme".to_string(),
                "templates.steps.qtctInstallStyle".to_string(),
            ];
        }
        "gtk-colors.css" => {
            meta.category = "KDE Plasma".to_string();
            meta.target_app = "GTK 3/4".to_string();
            meta.automation_level = "config-patch".to_string();
            meta.manual_steps = vec!["templates.steps.gtkImportColors".to_string()];
        }

        // Terminals
        "kitty-colors.conf" | "ghostty" | "alacritty.toml" | "wezterm_theme.toml" => {
            meta.category = "Terminals".to_string();
            meta.automation_level = "config-patch".to_string();
        }
        "terminal-sequences" => {
            meta.category = "Terminals".to_string();
            meta.automation_level = "config-patch".to_string();
        }

        // Shell / CLI productivity tools
        "tmux-colors.conf" => {
            meta.category = "Shell Tools".to_string();
            meta.automation_level = "config-patch".to_string();
        }
        "zellij-theme.kdl.tera" => {
            meta.category = "Shell Tools".to_string();
            meta.display_name = "Zellij".to_string();
            meta.target_app = "Zellij".to_string();
            meta.automation_level = "config-patch".to_string();
        }
        "mcfly.toml" => {
            meta.category = "Shell Tools".to_string();
            meta.display_name = "McFly".to_string();
            meta.target_app = "McFly".to_string();
        }
        "starship-colors.toml"
        | "yazi-theme.toml"
        | "television.toml"
        | "btop.theme"
        | "cava-colors.ini"
        | "opencode-colors.json"
        | "aerc" => {
            meta.category = "Shell Tools".to_string();
        }

        // Browsers
        "firefox-colors.css" | "pywalfox-colors.json" | "vivaldi.css" => {
            meta.category = "Browsers".to_string();
            meta.automation_level = "manual".to_string();
        }
        "zen-userchrome.css" | "zen-usercontent.css" => {
            meta.category = "Browsers".to_string();
            meta.display_name = if file_name == "zen-userchrome.css" {
                "Zen Browser (userChrome)".to_string()
            } else {
                "Zen Browser (userContent)".to_string()
            };
            meta.target_app = "Zen Browser".to_string();
            meta.automation_level = "manual".to_string();
            meta.manual_steps = vec![
                "templates.steps.zenEnableStylesheets".to_string(),
                "templates.steps.zenImportChrome".to_string(),
            ];
            meta.related_files = vec![
                "templates/zen-userchrome.css".to_string(),
                "templates/zen-usercontent.css".to_string(),
            ];
        }

        // Editors
        "nvim-colors.vim" | "template.lua" | "helix.toml" | "zed-colors.json" | "obsidian.css"
        | "micro.micro" => {
            meta.category = "Editors".to_string();
            meta.automation_level = if file_name == "helix.toml" {
                "config-patch".to_string()
            } else {
                "manual".to_string()
            };
        }
        "vscode-colors" | "vscode-colors.json" => {
            meta.category = "Editors".to_string();
            meta.display_name = "VS Code".to_string();
            meta.target_app = "VS Code".to_string();
            meta.automation_level = "manual".to_string();
            meta.manual_steps = vec!["templates.steps.vscodeInstallExtension".to_string()];
            meta.related_files = vec![
                "templates/vscode-colors".to_string(),
                "templates/vscode-colors.json".to_string(),
            ];
        }

        // Apps
        "midnight-discord.css" => {
            meta.display_name = "Discord (Midnight)".to_string();
            meta.target_app = "Discord".to_string();
            meta.automation_level = "manual".to_string();
        }
        "system24.css" => {
            meta.display_name = "Discord (System24)".to_string();
            meta.target_app = "Discord".to_string();
            meta.automation_level = "manual".to_string();
        }
        "spicetify.ini" => {
            meta.display_name = "Spotify (Spicetify)".to_string();
            meta.target_app = "Spotify".to_string();
            meta.automation_level = "manual".to_string();
        }
        "steam.css" => {
            meta.automation_level = "manual".to_string();
        }
        "telegram.tdesktop-theme" => {
            meta.display_name = "Telegram".to_string();
            meta.target_app = "Telegram".to_string();
            meta.automation_level = "manual".to_string();
        }
        "heroic.css" => {
            meta.target_app = "Heroic Games Launcher".to_string();
            meta.automation_level = "config-patch".to_string();
            meta.manual_steps = vec![
                "templates.steps.heroicCustomThemesPath".to_string(),
                "templates.steps.heroicSelectTheme".to_string(),
            ];
        }
        "foot-colors.ini" => {
            meta.category = "Terminals".to_string();
            meta.display_name = "Foot".to_string();
            meta.target_app = "Foot".to_string();
            meta.automation_level = "config-patch".to_string();
            meta.manual_steps = vec!["templates.steps.footIncludeConfig".to_string()];
        }
        "ghostwriter.json" => {
            meta.category = "Editors".to_string();
            meta.display_name = "Ghostwriter".to_string();
            meta.target_app = "Ghostwriter".to_string();
            meta.automation_level = "config-patch".to_string();
            meta.manual_steps = vec!["templates.steps.ghostwriterSetTheme".to_string()];
        }
        "prismlauncher.json" => {
            meta.display_name = "PrismLauncher".to_string();
            meta.target_app = "PrismLauncher".to_string();
            meta.automation_level = "manual".to_string();
        }
        "matugen.obt" => {
            meta.display_name = "OBS Studio".to_string();
            meta.target_app = "OBS Studio".to_string();
            meta.automation_level = "manual".to_string();
        }
        "rmpc.ron" => {
            meta.display_name = "Rmpc".to_string();
            meta.target_app = "Rmpc".to_string();
            meta.automation_level = "config-patch".to_string();
            meta.manual_steps = vec!["templates.steps.rmpcSetTheme".to_string()];
        }
        "wine.reg" => {
            meta.display_name = "Wine".to_string();
            meta.target_app = "Wine".to_string();
            meta.automation_level = "config-patch".to_string();
        }
        "zathura-colors" => {
            meta.display_name = "Zathura".to_string();
            meta.target_app = "Zathura".to_string();
        }
        "papirus-color" => {
            meta.display_name = "Papirus Folders".to_string();
            meta.target_app = "Papirus Icon Theme".to_string();
            meta.automation_level = "manual".to_string();
            meta.required_commands = vec!["papirus-folders".to_string()];
            meta.manual_steps = vec![
                "templates.steps.papirusInstall".to_string(),
                "templates.steps.papirusPostHook".to_string(),
            ];
        }
        _ => {}
    }

    if relative_path == "neovim/template.lua" {
        meta.display_name = "Neovim Lua".to_string();
        meta.target_app = "Neovim".to_string();
        meta.category = "Editors".to_string();
        meta.related_files = vec!["templates/neovim/init.lua".to_string()];
        meta.manual_steps = vec![
            "templates.steps.neovimBase16".to_string(),
            "templates.steps.neovimReloadHook".to_string(),
        ];
    }

    meta
}

fn friendly_name(file_name: &str) -> String {
    let name = file_name
        .trim_end_matches(".tera")
        .trim_end_matches(".toml")
        .trim_end_matches(".conf")
        .trim_end_matches(".json")
        .trim_end_matches(".rasi")
        .trim_end_matches(".css")
        .trim_end_matches(".ini")
        .trim_end_matches(".theme")
        .trim_end_matches(".colors")
        .replace(['-', '_'], " ");

    name.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn target_from_filename(file_name: &str) -> String {
    friendly_name(file_name)
        .replace(" Colors", "")
        .replace(" Theme", "")
}

fn default_output_path(file_name: &str) -> Option<&'static str> {
    match file_name {
        "Matugen.colors" => Some("~/.local/share/color-schemes/Matugen.colors"),
        "aerc" => Some("~/.config/aerc/stylesets/matugen"),
        "alacritty.toml" => Some("~/.config/alacritty/colors.toml"),
        "btop.theme" => Some("~/.config/btop/themes/matugen.theme"),
        "cava-colors.ini" => Some("~/.config/cava/themes/matugen"),
        "firefox-colors.css" => Some("~/.cache/matugen/firefox/colors.css"),
        "foot-colors.ini" => Some("~/.config/foot/foot-colors.ini"),
        "ghostty" => Some("~/.config/ghostty/themes/Matugen"),
        "ghostwriter.json" => Some("~/.local/share/ghostwriter/themes/Matugen.json"),
        "gtk-colors.css" => Some("~/.config/gtk-4.0/colors.css"),
        "helix.toml" => Some("~/.config/helix/themes/matugen.toml"),
        "heroic.css" => Some("~/.config/heroic/themes/matugen.css"),
        "kitty-colors.conf" => Some("~/.config/kitty/themes/Matugen.conf"),
        "kvantum-colors.kvconfig" => Some("~/.config/Kvantum/matugen/matugen.kvconfig"),
        "kvantum-colors.svg" => Some("~/.config/Kvantum/matugen/matugen.svg"),
        "matugen.obt" => Some("~/.config/obs-studio/themes/matugen.obt"),
        "mcfly.toml" => Some("~/.local/share/mcfly/config.toml"),
        "micro.micro" => Some("~/.config/micro/colorschemes/matugen.micro"),
        "midnight-discord.css" => Some("~/.config/vesktop/themes/midnight-discord.css"),
        "nvim-colors.vim" => Some("~/.config/nvim/colors/matugen.vim"),
        "obsidian.css" => Some("~/Documents/ObsidianVault/.obsidian/snippets/matugen.css"),
        "opencode-colors.json" => Some("~/.config/opencode/themes/matugen.json"),
        "papirus-color" => Some("~/.cache/matugen/papirus-color"),
        "prismlauncher.json" => Some("~/.local/share/PrismLauncher/themes/Matugen/theme.json"),
        "pywalfox-colors.json" => Some("~/.cache/wal/colors.json"),
        "qtct-colors.conf" => Some("~/.config/qt6ct/colors/matugen.conf"),
        "rmpc.ron" => Some("~/.config/rmpc/themes/matugen.ron"),
        "spicetify.ini" => Some("~/.config/spicetify/Themes/Sleek/color.ini"),
        "starship-colors.toml" => Some("~/.config/starship.toml"),
        "steam.css" => Some("~/.config/AdwSteamGtk/custom.css"),
        "system24.css" => Some("~/.config/vesktop/themes/system24.css"),
        "telegram.tdesktop-theme" => Some("~/Downloads/matugen.tdesktop-theme"),
        "television.toml" => Some("~/.config/television/themes/matugen.toml"),
        "terminal-sequences" => Some("~/.cache/terminal-sequences"),
        "tmux-colors.conf" => Some("~/.config/tmux/generated.conf"),
        "vivaldi.css" => Some("~/.config/vivaldi-matugen/vivaldi.css"),
        "vscode-colors" => Some("~/.cache/matugen/vscode-colors"),
        "vscode-colors.json" => Some("~/.cache/matugen/vscode-colors.json"),
        "wezterm_theme.toml" => Some("~/.config/wezterm/colors/matugen_theme.toml"),
        "wine.reg" => Some("/tmp/wine.reg"),
        "yazi-theme.toml" => Some("~/.config/yazi/theme.toml"),
        "zathura-colors" => Some("~/.config/zathura/zathurarc"),
        "zed-colors.json" => Some("~/.config/zed/themes/matugen.json"),
        "zellij-theme.kdl.tera" => Some("~/.config/zellij/themes/matugen.kdl"),
        _ => None,
    }
}

fn default_post_hook(file_name: &str) -> Option<&'static str> {
    match file_name {
        "Matugen.colors" => Some("plasma-apply-colorscheme Matugen"),
        "btop.theme" => Some("pkill -USR2 btop || true"),
        "foot-colors.ini" => Some("pkill -SIGUSR1 foot || true"),
        "ghostty" => Some("pkill -SIGUSR2 ghostty"),
        "kitty-colors.conf" => Some("kitty +kitten themes --reload-in=all Matugen"),
        "nvim-colors.vim" | "template.lua" => Some("pkill -SIGUSR1 nvim"),
        "pywalfox-colors.json" => Some("pywalfox update"),
        "spicetify.ini" => Some("spicetify watch -s 2>&1 | sed \"/Reloaded Spotify/q\""),
        "steam.css" => Some("adwaita-steam-gtk -i"),
        "terminal-sequences" => Some("cat ~/.cache/terminal-sequences > /dev/pts/[0-9]*"),
        "tmux-colors.conf" => Some("tmux source-file ~/.config/tmux/generated.conf"),
        "wezterm_theme.toml" => Some("touch ~/.config/wezterm/wezterm.lua"),
        "wine.reg" => Some("wine regedit /tmp/wine.reg"),
        "zellij-theme.kdl.tera" => Some("touch ~/.config/zellij/config.kdl"),
        _ => None,
    }
}

fn manual_steps(file_name: &str) -> Vec<String> {
    match file_name {
        "aerc" => vec!["templates.steps.aercStyleset".to_string()],
        "alacritty.toml" => vec!["templates.steps.alacrittyImport".to_string()],
        "btop.theme" => vec!["templates.steps.btopChooseTheme".to_string()],
        "cava-colors.ini" => vec!["templates.steps.cavaSetTheme".to_string()],
        "foot-colors.ini" => vec!["templates.steps.footIncludeConfig".to_string()],
        "ghostty" => vec!["templates.steps.ghosttySetTheme".to_string()],
        "ghostwriter.json" => vec!["templates.steps.ghostwriterSetTheme".to_string()],
        "helix.toml" => vec!["templates.steps.helixSetTheme".to_string()],
        "heroic.css" => vec![
            "templates.steps.heroicCustomThemesPath".to_string(),
            "templates.steps.heroicSelectTheme".to_string(),
        ],
        "kitty-colors.conf" => vec!["templates.steps.kittyApplyTheme".to_string()],
        "midnight-discord.css" | "system24.css" => {
            vec!["templates.steps.discordActivate".to_string()]
        }
        "spicetify.ini" => vec![
            "templates.steps.spicetifyConfig".to_string(),
            "templates.steps.spicetifyDownloadSleek".to_string(),
        ],
        "steam.css" => vec![
            "templates.steps.steamInstallAdwSteamGtk".to_string(),
            "templates.steps.steamEnableCustomCss".to_string(),
        ],
        "telegram.tdesktop-theme" => vec![
            "templates.steps.telegramManualIntro".to_string(),
            "templates.steps.telegramApply".to_string(),
        ],
        "vivaldi.css" => vec![
            "templates.steps.vivaldiEnableExperiment".to_string(),
            "templates.steps.vivaldiSelectFolder".to_string(),
        ],
        "zellij-theme.kdl.tera" => vec!["templates.steps.zellijAddTheme".to_string()],
        _ => vec![],
    }
}

fn bundled_themes_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let resource_path = app
        .path()
        .resolve(
            "resources/matugen-themes",
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| format!("Failed to resolve bundled themes path: {}", e))?;

    if resource_path.exists() {
        return Ok(resource_path);
    }

    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("matugen-themes");

    if dev_path.exists() {
        return Ok(dev_path);
    }

    Err(format!(
        "Bundled matugen-themes directory not found at {}",
        resource_path.display()
    ))
}

#[tauri::command]
pub fn list_bundled_templates(app: tauri::AppHandle) -> Result<Vec<TemplateInfo>, String> {
    list_templates_from_themes_dir(bundled_themes_dir(&app)?)
}

#[tauri::command]
pub fn list_available_templates(themes_dir: String) -> Result<Vec<TemplateInfo>, String> {
    list_templates_from_themes_dir(PathBuf::from(themes_dir))
}

struct CatalogPaths {
    home: PathBuf,
    config: PathBuf,
    data: PathBuf,
    cache: PathBuf,
    state: PathBuf,
}

impl CatalogPaths {
    fn system() -> Result<Self, String> {
        let home = dirs::home_dir().ok_or("Could not resolve home directory")?;
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home.join(".local/state"));
        Ok(Self {
            home,
            config: dirs::config_dir().ok_or("Could not resolve config directory")?,
            data: dirs::data_dir().ok_or("Could not resolve data directory")?,
            cache: dirs::cache_dir().ok_or("Could not resolve cache directory")?,
            state,
        })
    }

    fn resolve(&self, expression: &str) -> Result<PathBuf, String> {
        for (prefix, base) in [
            ("$XDG_CONFIG_HOME/", &self.config),
            ("$XDG_DATA_HOME/", &self.data),
            ("$XDG_CACHE_HOME/", &self.cache),
            ("$XDG_STATE_HOME/", &self.state),
            ("$HOME/", &self.home),
            ("~/", &self.home),
        ] {
            if let Some(rest) = expression.strip_prefix(prefix) {
                if rest.is_empty()
                    || rest.starts_with('/')
                    || rest
                        .split('/')
                        .any(|component| component == ".." || component == ".")
                {
                    return Err(format!("Invalid catalog path: {expression}"));
                }
                return Ok(base.join(rest));
            }
        }
        Err(format!("Unsupported catalog path: {expression}"))
    }
}

fn catalog_slug(value: &str) -> String {
    let mut slug = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    slug.trim_end_matches('-').to_string()
}

fn normalized_category<'a>(category: &'a str, app_name: &str) -> &'a str {
    match app_name {
        "Discord" | "Telegram" => "Communication",
        "Spotify" | "OBS Studio" => "Media",
        "Heroic Games Launcher" | "Steam" | "PrismLauncher" => "Gaming",
        "Ghostwriter" => "Productivity",
        "Btop" => "System",
        _ => match category {
            "KDE Plasma" => "Desktop",
            "Terminals" => "Terminals",
            "Shell Tools" | "Editors" => "Development",
            "Browsers" => "Browsers",
            "Apps" => "Utilities",
            other => other,
        },
    }
}

fn bundled_source(relative_path: &str) -> TemplateSource {
    TemplateSource {
        kind: "official".into(),
        name: "matugen-themes".into(),
        repository: Some("https://github.com/InioX/matugen-themes".into()),
        author: None,
        license: Some("MIT".into()),
        license_status: Some("repository-declared".into()),
        source_path: Some(if relative_path.starts_with("websites/") {
            relative_path.to_string()
        } else {
            format!("templates/{relative_path}")
        }),
        upstream_commit: None,
        version: None,
        attribution: None,
    }
}

fn list_template_catalog_from_dir(
    themes_dir: PathBuf,
    paths: &CatalogPaths,
) -> Result<Vec<TemplateCatalogApp>, String> {
    let templates = list_templates_from_themes_dir(themes_dir)?;
    let metadata: CatalogMetadata =
        serde_json::from_str(include_str!("../../resources/matugen-themes/catalog.json"))
            .map_err(|e| format!("Invalid bundled catalog metadata: {e}"))?;
    let by_path: HashMap<&str, &TemplateInfo> = templates
        .iter()
        .map(|template| (template.relative_path.as_str(), template))
        .collect();
    let mut consumed = HashSet::new();
    let mut catalog: Vec<TemplateCatalogApp> = Vec::new();

    for app in metadata.applications {
        let mut variants = Vec::new();
        for variant in app.variants {
            let Some(template) = by_path.get(variant.source_path.as_str()) else {
                continue;
            };
            let mut targets = Vec::new();
            for target in variant.targets {
                let source_path = target
                    .source_path
                    .as_deref()
                    .unwrap_or(&variant.source_path);
                let Some(input) = by_path.get(source_path) else {
                    continue;
                };
                consumed.insert(source_path.to_string());
                let output = paths.resolve(&target.output_path)?;
                let detected = match &target.detect_path {
                    Some(path) => paths.resolve(path)?.exists(),
                    None => output.exists(),
                };
                targets.push(TemplateTarget {
                    id: target.id,
                    label: target.label,
                    install_type: target.install_type,
                    input_path: input.path.clone(),
                    output_path: output.to_string_lossy().into_owned(),
                    detected,
                });
            }
            if targets.is_empty() {
                continue;
            }
            consumed.insert(variant.source_path.clone());
            let source = match variant.source {
                Some(meta) => {
                    if meta.kind != "official" && meta.kind != "community" {
                        return Err(format!("Invalid source kind for {}", variant.id));
                    }
                    TemplateSource {
                        kind: meta.kind,
                        name: meta.name,
                        repository: meta.repository,
                        author: meta.author,
                        license: meta.license,
                        license_status: meta.license_status,
                        source_path: Some(format!("templates/{}", variant.source_path)),
                        upstream_commit: meta.upstream_commit,
                        version: meta.version,
                        attribution: meta.attribution,
                    }
                }
                None => bundled_source(&variant.source_path),
            };
            let mut template = (*template).clone();
            template.target_app = app.name.clone();
            template.category = normalized_category(&app.category, &app.name).into();
            if let Some(level) = variant.automation_level {
                if !["auto", "config-patch", "manual"].contains(&level.as_str()) {
                    return Err(format!("Invalid automation level for {}", variant.id));
                }
                template.automation_level = level;
            }
            if let Some(steps) = variant.manual_steps {
                if steps
                    .iter()
                    .any(|step| !step.starts_with("templates.steps."))
                {
                    return Err(format!("Invalid manual step for {}", variant.id));
                }
                template.manual_steps = steps;
            }
            variants.push(TemplateCatalogVariant {
                id: variant.id,
                name: variant.name,
                description: variant.description,
                source,
                targets,
                template,
            });
        }
        if !variants.is_empty() {
            catalog.push(TemplateCatalogApp {
                id: app.id,
                category: normalized_category(&app.category, &app.name).into(),
                name: app.name,
                variants,
            });
        }
    }

    // Discovery remains authoritative: every bundled file not represented by the
    // curated groups still appears once, including future upstream additions.
    for template in &templates {
        if consumed.contains(&template.relative_path) {
            continue;
        }
        let app_name = &template.target_app;
        let index = catalog.iter().position(|app| app.name == *app_name);
        let index = match index {
            Some(index) => index,
            None => {
                catalog.push(TemplateCatalogApp {
                    id: catalog_slug(app_name),
                    name: app_name.clone(),
                    category: normalized_category(&template.category, app_name).into(),
                    variants: Vec::new(),
                });
                catalog.len() - 1
            }
        };
        let output = template
            .default_output_path
            .as_deref()
            .map(|path| resolve_catalog_legacy_path(paths, path))
            .transpose()?
            .unwrap_or_else(|| {
                paths
                    .cache
                    .join("matugen/templates")
                    .join(template.relative_path.replace('/', "-"))
            });
        let detected = output.exists();
        let variant_id = format!(
            "{}-{}",
            catalog[index].id,
            catalog_slug(&template.relative_path)
        );
        catalog[index].variants.push(TemplateCatalogVariant {
            id: variant_id,
            name: template.display_name.clone(),
            description: None,
            source: bundled_source(&template.relative_path),
            targets: vec![TemplateTarget {
                id: "default".into(),
                label: "Default".into(),
                install_type: if template.installable {
                    "native"
                } else {
                    "manual"
                }
                .into(),
                input_path: template.path.clone(),
                output_path: output.to_string_lossy().into_owned(),
                detected,
            }],
            template: template.clone(),
        });
    }
    catalog.sort_by(|a, b| a.category.cmp(&b.category).then(a.name.cmp(&b.name)));
    Ok(catalog)
}

fn resolve_catalog_legacy_path(paths: &CatalogPaths, path: &str) -> Result<PathBuf, String> {
    if let Some(rest) = path.strip_prefix("~/.config/") {
        Ok(paths.config.join(rest))
    } else if let Some(rest) = path.strip_prefix("~/.local/share/") {
        Ok(paths.data.join(rest))
    } else if let Some(rest) = path.strip_prefix("~/.cache/") {
        Ok(paths.cache.join(rest))
    } else if let Some(rest) = path.strip_prefix("~/") {
        Ok(paths.home.join(rest))
    } else {
        Ok(PathBuf::from(path))
    }
}

#[tauri::command]
pub fn list_template_catalog(app: tauri::AppHandle) -> Result<Vec<TemplateCatalogApp>, String> {
    list_template_catalog_from_dir(bundled_themes_dir(&app)?, &CatalogPaths::system()?)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HeroicDetectionResult {
    pub native_detected: bool,
    pub flatpak_detected: bool,
    pub native_path: String,
    pub flatpak_path: String,
    pub native_themes_dir: String,
    pub flatpak_themes_dir: String,
}

pub fn detect_heroic_installation_in(config_dir: &Path, home_dir: &Path) -> HeroicDetectionResult {
    let native_config = config_dir.join("heroic");
    let native_detected = native_config.is_dir();
    let flatpak_root = home_dir.join(".var/app/com.heroicgameslauncher.hgl");
    let flatpak_detected = flatpak_root.is_dir();

    let native_themes_dir = if let Ok(stripped) = config_dir.strip_prefix(home_dir) {
        format!("~/{}/heroic/themes", stripped.display())
    } else {
        format!("{}/heroic/themes", config_dir.display())
    };
    let native_path = format!("{}/matugen.css", native_themes_dir);

    HeroicDetectionResult {
        native_detected,
        flatpak_detected,
        native_path,
        flatpak_path: "~/.var/app/com.heroicgameslauncher.hgl/config/heroic/themes/matugen.css"
            .to_string(),
        native_themes_dir,
        flatpak_themes_dir: "~/.var/app/com.heroicgameslauncher.hgl/config/heroic/themes"
            .to_string(),
    }
}

#[tauri::command]
pub fn detect_heroic_installation() -> Result<HeroicDetectionResult, String> {
    let config_dir = dirs::config_dir().ok_or("Could not resolve config directory")?;
    let home_dir = dirs::home_dir().ok_or("Could not resolve home directory")?;
    Ok(detect_heroic_installation_in(&config_dir, &home_dir))
}

#[tauri::command]
pub fn preview_template(
    app: tauri::AppHandle,
    template_path: String,
    context: serde_json::Value,
) -> Result<String, String> {
    let requested = PathBuf::from(&template_path)
        .canonicalize()
        .map_err(|e| format!("Failed to resolve template path: {}", e))?;
    let bundled = bundled_themes_dir(&app)?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let user_templates = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("matugen/templates");
    let allowed_user_path = user_templates
        .canonicalize()
        .map(|root| requested.starts_with(root))
        .unwrap_or(false);
    if !requested.starts_with(&bundled) && !allowed_user_path {
        return Err("Template preview is restricted to managed template directories".to_string());
    }

    let mut engine = Engine::new();
    State::add_engine_filters(&mut engine);
    let source =
        fs::read_to_string(&requested).map_err(|e| format!("Failed to read template: {}", e))?;
    engine
        .add_template("preview".to_string(), source)
        .map_err(|error| format!("Failed to parse template: {}", error))?;
    engine
        .add_context(context)
        .map_err(|error| format!("Invalid template context: {}", error))?;
    engine.render("preview").map_err(|errors| {
        errors
            .into_iter()
            .map(|error| format!("{:?}", error))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

#[tauri::command]
pub fn install_template(
    template_path: String,
    template_name: String,
    output_path: String,
    post_hook: Option<String>,
) -> Result<(), String> {
    if output_path.trim().is_empty() {
        return Err("Output path cannot be empty for this template".to_string());
    }
    let _guard = TEMPLATE_STORAGE_LOCK.lock().map_err(|e| e.to_string())?;
    install_template_in(
        &matugen_config_dir()?,
        &template_path,
        &template_name,
        output_path.trim(),
        post_hook.as_deref(),
        &CatalogPaths::system()?,
    )
}

fn safe_template_name(name: &str) -> Result<&str, String> {
    Path::new(name)
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .filter(|file_name| *file_name == name && !name.contains("..") && !name.is_empty())
        .ok_or_else(|| "Invalid template name".to_string())
}

fn installed_key(name: &str) -> String {
    if name.contains("__") {
        name.to_string()
    } else {
        sanitize_template_key(name)
    }
}

fn template_table(content: &str) -> Result<toml::Value, String> {
    let document = if content.contains("[config]") {
        content.to_string()
    } else {
        format!("[config]\n{content}")
    };
    toml::from_str(&document).map_err(|e| format!("Invalid config.toml: {e}"))
}

fn portable_output(path: &str, paths: &CatalogPaths) -> Result<String, String> {
    if path.starts_with("$XDG_") || path.starts_with("$HOME/") || path.starts_with("~/") {
        Ok(paths.resolve(path)?.to_string_lossy().into_owned())
    } else {
        Ok(path.to_string())
    }
}
fn normalized_output(path: &str, config_dir: &Path) -> PathBuf {
    use std::path::Component;
    let expanded = expand_tilde(Path::new(path));
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        config_dir.join(expanded)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn install_template_in(
    config_dir: &Path,
    template_path: &str,
    template_name: &str,
    output_path: &str,
    post_hook: Option<&str>,
    paths: &CatalogPaths,
) -> Result<(), String> {
    let output_path = portable_output(output_path, paths)?;
    let safe_name = safe_template_name(template_name)?;
    let name_key = installed_key(safe_name);
    let templates_dir = config_dir.join("templates");
    let dest_path = templates_dir.join(safe_name);
    let source = Path::new(template_path);
    let config_path = config_dir.join("config.toml");
    let mut config_content = if config_path.exists() {
        fs::read_to_string(&config_path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let parsed = template_table(&config_content)?;
    let requested_output = normalized_output(&output_path, config_dir);
    if let Some(entries) = parsed.get("templates").and_then(toml::Value::as_table) {
        if entries.contains_key(&name_key) {
            return Err("Template already installed. Please check your matugen config.toml".into());
        }
        // A user-authored entry may have several outputs; none can be claimed
        // by a different installation, even before a file is rendered.
        let is_same_output = |path: &str| {
            portable_output(path, paths)
                .is_ok_and(|resolved| normalized_output(&resolved, config_dir) == requested_output)
        };
        if let Some((other, _)) = entries.iter().find(|(other, entry)| {
            *other != &name_key
                && entry.get("output_path").is_some_and(|output| match output {
                    toml::Value::String(path) => is_same_output(path),
                    toml::Value::Array(values) => values
                        .iter()
                        .any(|value| value.as_str().is_some_and(is_same_output)),
                    _ => false,
                })
        }) {
            return Err(format!(
                "Output path is already used by installed template '{other}'"
            ));
        }
    }
    match fs::symlink_metadata(&requested_output) {
        Ok(_) => {
            return Err(format!(
                "Output path already exists and is not managed by this installation: {}",
                requested_output.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Cannot inspect output path: {error}")),
    }
    if source == dest_path {
        return Err("Managed template input already exists; refusing to overwrite it".into());
    }
    match fs::symlink_metadata(&dest_path) {
        Ok(_) => {
            return Err("Managed template input already exists; refusing to overwrite it".into())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Cannot inspect managed template input: {error}")),
    }
    if !config_content.contains("[config]") {
        config_content = format!("[config]\n{config_content}");
    }
    config_content.push_str(&format!(
        "\n[templates.\"{}\"]\ninput_path = \"{}\"\noutput_path = \"{}\"\n",
        escape_toml_string(&name_key),
        escape_toml_string(&dest_path.to_string_lossy()),
        escape_toml_string(&requested_output.to_string_lossy()),
    ));
    if let Some(hook) = post_hook.filter(|hook| !hook.trim().is_empty()) {
        config_content.push_str(&format!(
            "post_hook = \"{}\"\n",
            escape_toml_string(hook.trim())
        ));
    }
    template_table(&config_content)?;
    fs::create_dir_all(&templates_dir).map_err(|e| e.to_string())?;
    if let Err(error) = fs::copy(source, &dest_path) {
        let _ = fs::remove_file(&dest_path);
        return Err(error.to_string());
    }
    if let Err(error) = atomic_write_text(&config_path, &config_content) {
        let _ = fs::remove_file(&dest_path);
        return Err(error);
    }
    Ok(())
}

fn escape_toml_string(s: &str) -> String {
    s.chars()
        .filter(|&c| c != '\r')
        .fold(String::with_capacity(s.len()), |mut acc, c| {
            match c {
                '\\' => acc.push_str("\\\\"),
                '"' => acc.push_str("\\\""),
                '\n' => acc.push_str("\\n"),
                _ => acc.push(c),
            }
            acc
        })
}

fn atomic_write_text(path: &Path, content: &str) -> Result<(), String> {
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content).map_err(|e| e.to_string())?;
    fs::rename(&temp_path, path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        e.to_string()
    })
}

fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

fn matugen_config_dir() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("matugen"))
}

fn studio_config_dir() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("matugen-studio"))
}

fn overrides_path() -> Result<PathBuf, String> {
    Ok(studio_config_dir()?.join("template-overrides.json"))
}

fn read_template_overrides() -> Result<TemplateOverrides, String> {
    let path = overrides_path()?;
    if !path.exists() {
        return Ok(TemplateOverrides::default());
    }

    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn write_template_overrides(overrides: &TemplateOverrides) -> Result<(), String> {
    let config_dir = studio_config_dir()?;
    fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let path = config_dir.join("template-overrides.json");
    let temp_path = config_dir.join("template-overrides.json.tmp");
    let content = serde_json::to_string_pretty(overrides).map_err(|e| e.to_string())?;
    fs::write(&temp_path, content).map_err(|e| e.to_string())?;
    fs::rename(&temp_path, &path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        e.to_string()
    })
}

fn hex_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)#(?:[0-9a-f]{8}|[0-9a-f]{6})").unwrap())
}

fn matugen_ref_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"colors\.([A-Za-z0-9_]+)\.([A-Za-z0-9_]+)\.(hex|color)").unwrap())
}

fn validate_hex(hex: &str) -> Result<String, String> {
    let value = hex.trim();
    if value.len() != 7 || !value.starts_with('#') {
        return Err("Color must use #RRGGBB format".to_string());
    }

    if value[1..].chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(value.to_uppercase())
    } else {
        Err("Color must use #RRGGBB format".to_string())
    }
}

fn config_file_and_path() -> Result<Option<(ConfigFile, PathBuf)>, String> {
    let config_dir = matugen_config_dir()?;
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        return Ok(None);
    }

    let mut config_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.toml: {}", e))?;

    if !config_content.contains("[config]") {
        config_content = "[config]\n".to_string() + &config_content;
    }

    let config_file: ConfigFile =
        toml::from_str(&config_content).map_err(|e| format!("Invalid config.toml: {}", e))?;
    Ok(Some((config_file, config_path)))
}

fn resolve_template_input(config_path: &Path, input_path: &PathBuf) -> PathBuf {
    let input_abs = if input_path.is_absolute() {
        input_path.clone()
    } else {
        config_path.parent().unwrap().join(input_path)
    };

    expand_tilde(&input_abs)
}

fn declaration_from_line(line: &str) -> Option<ParsedDeclaration> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
        return None;
    }

    let separator = if trimmed.starts_with("--") {
        ':'
    } else if trimmed.contains('=') {
        '='
    } else if trimmed.contains(':') {
        ':'
    } else {
        return None;
    };

    let separator_index = trimmed.find(separator)?;
    let raw_key = trimmed[..separator_index]
        .trim()
        .trim_matches('"')
        .trim_matches('\'');

    if raw_key.is_empty() || raw_key.starts_with('[') || raw_key.len() > 80 {
        return None;
    }

    let mut value_part = trimmed[separator_index + 1..].trim();
    let mut description = None;

    if let (Some(start), Some(end)) = (value_part.find("/*"), value_part.find("*/")) {
        if end > start {
            description = Some(value_part[start + 2..end].trim().to_string());
            value_part = value_part[..start].trim();
        }
    } else if !trimmed.starts_with("--") {
        if let Some(comment_index) = value_part.find(" #") {
            description = Some(value_part[comment_index + 2..].trim().to_string());
            value_part = value_part[..comment_index].trim();
        }
    }

    value_part = value_part
        .trim_end_matches(',')
        .trim_end_matches(';')
        .trim()
        .trim_matches('"')
        .trim_matches('\'');

    Some(ParsedDeclaration {
        key: raw_key.to_string(),
        value: value_part.to_string(),
        description: description.filter(|text| !text.is_empty()),
    })
}

fn friendly_token_name(key: &str) -> String {
    key.trim_start_matches("--")
        .trim_matches('"')
        .trim_matches('\'')
        .replace(['-', '_'], " ")
}

fn source_path_from_value(value: &str) -> Option<String> {
    matugen_ref_regex().captures(value).map(|captures| {
        format!(
            "{}.{}.{}",
            captures.get(1).map(|m| m.as_str()).unwrap_or_default(),
            captures.get(2).map(|m| m.as_str()).unwrap_or_default(),
            captures.get(3).map(|m| m.as_str()).unwrap_or_default()
        )
    })
}

fn rendered_declarations(rendered: &str) -> HashMap<String, String> {
    let mut declarations = HashMap::new();
    for line in rendered.lines() {
        if let Some(declaration) = declaration_from_line(line) {
            declarations
                .entry(declaration.key)
                .or_insert(declaration.value);
        }
    }
    declarations
}

fn parse_template_color_controls(source: &str, rendered: &str) -> Vec<ParsedColorControl> {
    let rendered_values = rendered_declarations(rendered);
    let mut controls = Vec::new();
    let mut seen = HashMap::new();

    for line in source.lines() {
        if !line.contains("{{") || !line.contains("colors.") {
            continue;
        }

        let Some(declaration) = declaration_from_line(line) else {
            continue;
        };
        let Some(source_path) = source_path_from_value(&declaration.value) else {
            continue;
        };
        let Some(rendered_value) = rendered_values.get(&declaration.key) else {
            continue;
        };
        let Some(hex_match) = hex_regex().find(rendered_value) else {
            continue;
        };

        if seen.insert(declaration.key.clone(), true).is_some() {
            continue;
        }

        controls.push(ParsedColorControl {
            name: friendly_token_name(&declaration.key),
            key: declaration.key,
            description: declaration.description,
            source_path: Some(source_path),
            original_hex: hex_match.as_str().to_uppercase(),
        });
    }

    controls
}

fn apply_template_overrides(
    template_name: &str,
    rendered: &str,
    overrides: &TemplateOverrides,
) -> String {
    let Some(template_overrides) = overrides.templates.get(template_name) else {
        return rendered.to_string();
    };

    let mut output = String::with_capacity(rendered.len());
    for line in rendered.lines() {
        if let Some(declaration) = declaration_from_line(line) {
            if let Some(color_override) = template_overrides.get(&declaration.key) {
                if let Some(hex_match) = hex_regex().find(line) {
                    let hex = color_override.hex();
                    let original = hex_match.as_str();
                    let replacement = if original.len() == 9 {
                        format!("{}{}", hex, &original[7..])
                    } else {
                        hex.to_string()
                    };
                    output.push_str(&line.replacen(original, &replacement, 1));
                    output.push('\n');
                    continue;
                }
            }
        }
        output.push_str(line);
        output.push('\n');
    }

    output
}

fn render_template(
    name: &str,
    source: String,
    context: serde_json::Value,
) -> Result<String, String> {
    let mut engine = Engine::new();
    State::add_engine_filters(&mut engine);
    engine
        .add_context(context)
        .map_err(|error| format!("Invalid template context: {}", error))?;
    engine
        .add_template(name.to_string(), source)
        .map_err(|error| format!("Failed to parse template '{}': {}", name, error))?;

    engine.render(name).map_err(|errs| {
        let mut err_msg = String::new();
        for err in errs {
            err_msg.push_str(&format!("{:?}\n", err));
        }
        err_msg
    })
}

#[tauri::command]
pub async fn list_template_color_controls(
    context: Option<serde_json::Value>,
) -> Result<Vec<TemplateColorGroup>, String> {
    tauri::async_runtime::spawn_blocking(move || list_template_color_controls_blocking(context))
        .await
        .map_err(|e| e.to_string())?
}

fn list_template_color_controls_blocking(
    context: Option<serde_json::Value>,
) -> Result<Vec<TemplateColorGroup>, String> {
    let Some((config_file, config_path)) = config_file_and_path()? else {
        return Ok(vec![]);
    };
    let overrides = read_template_overrides()?;
    let mut groups = Vec::new();

    for (name, template) in config_file.templates.iter() {
        let input_abs = resolve_template_input(&config_path, &template.input_path);
        if !input_abs.exists() {
            continue;
        }

        let source = match fs::read_to_string(&input_abs) {
            Ok(source) => source,
            Err(_) => continue,
        };
        let rendered = if let Some(context) = context.clone() {
            match render_template(name, source.clone(), context) {
                Ok(rendered) => rendered,
                Err(_) => continue,
            }
        } else if let Some(matugen_core::template::OutputPath::Single(out_path)) =
            &template.output_path
        {
            let out_abs = expand_tilde(out_path);
            match fs::read_to_string(out_abs) {
                Ok(rendered) => rendered,
                Err(_) => continue,
            }
        } else {
            continue;
        };
        let controls = parse_template_color_controls(&source, &rendered);
        if controls.is_empty() {
            continue;
        }

        let file_name = input_abs
            .file_name()
            .and_then(|file| file.to_str())
            .unwrap_or(name);
        let meta = template_metadata(file_name, file_name, true);
        let template_overrides = overrides.templates.get(name);
        let controls = controls
            .into_iter()
            .map(|control| {
                let override_hex = template_overrides.and_then(|values| values.get(&control.key));
                let true_original = override_hex
                    .and_then(|value| value.original_hex())
                    .unwrap_or(&control.original_hex)
                    .to_string();
                TemplateColorControl {
                    key: control.key,
                    name: control.name,
                    description: control.description,
                    source_path: control.source_path,
                    current_hex: override_hex
                        .map(|value| value.hex().to_string())
                        .unwrap_or_else(|| true_original.clone()),
                    original_hex: true_original,
                    overridden: override_hex.is_some(),
                }
            })
            .collect::<Vec<_>>();

        let output_path = match &template.output_path {
            Some(matugen_core::template::OutputPath::Single(path)) => {
                Some(path.to_string_lossy().to_string())
            }
            _ => None,
        };

        groups.push(TemplateColorGroup {
            template_name: name.clone(),
            display_name: meta.display_name,
            target_app: meta.target_app,
            output_path,
            controls,
        });
    }

    groups.sort_by(|a, b| {
        a.target_app
            .cmp(&b.target_app)
            .then(a.display_name.cmp(&b.display_name))
    });
    Ok(groups)
}

#[tauri::command]
pub fn set_template_color_override(
    template_name: String,
    token_key: String,
    hex: String,
    original_hex: Option<String>,
) -> Result<(), String> {
    let _guard = TEMPLATE_STORAGE_LOCK.lock().map_err(|e| e.to_string())?;
    let hex = validate_hex(&hex)?;
    let mut overrides = read_template_overrides()?;
    overrides
        .templates
        .entry(template_name)
        .or_default()
        .insert(
            token_key,
            TemplateColorOverride::Detailed { hex, original_hex },
        );
    write_template_overrides(&overrides)
}

#[tauri::command]
pub fn reset_template_color_override(
    template_name: String,
    token_key: Option<String>,
) -> Result<(), String> {
    let _guard = TEMPLATE_STORAGE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut overrides = read_template_overrides()?;
    let mut restore_values = HashMap::new();
    if let Some(token_key) = token_key {
        if let Some(values) = overrides.templates.get_mut(&template_name) {
            if let Some(color_override) = values.remove(&token_key) {
                if let Some(original_hex) = color_override.original_hex() {
                    restore_values.insert(token_key, original_hex.to_string());
                }
            }
            if values.is_empty() {
                overrides.templates.remove(&template_name);
            }
        }
    } else {
        if let Some(values) = overrides.templates.remove(&template_name) {
            for (token_key, color_override) in values {
                if let Some(original_hex) = color_override.original_hex() {
                    restore_values.insert(token_key, original_hex.to_string());
                }
            }
        }
    }

    write_template_overrides(&overrides)?;
    if !restore_values.is_empty() {
        patch_template_output_tokens(&template_name, &restore_values)?;
    }
    Ok(())
}

fn patch_template_output_tokens(
    template_name: &str,
    values: &HashMap<String, String>,
) -> Result<(), String> {
    let Some((config_file, _config_path)) = config_file_and_path()? else {
        return Ok(());
    };
    let Some(template) = config_file.templates.get(template_name) else {
        return Ok(());
    };
    let Some(matugen_core::template::OutputPath::Single(out_path)) = &template.output_path else {
        return Ok(());
    };

    let out_abs = expand_tilde(out_path);
    if !out_abs.exists() {
        return Ok(());
    }

    let rendered = fs::read_to_string(&out_abs).map_err(|e| e.to_string())?;
    let mut overrides = TemplateOverrides::default();
    overrides.templates.insert(
        template_name.to_string(),
        values
            .iter()
            .map(|(key, hex)| (key.clone(), TemplateColorOverride::Legacy(hex.clone())))
            .collect(),
    );
    let patched = apply_template_overrides(template_name, &rendered, &overrides);
    fs::write(out_abs, patched).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn apply_template_overrides_to_outputs() -> Result<(), String> {
    let Some((config_file, _config_path)) = config_file_and_path()? else {
        return Ok(());
    };
    let overrides = read_template_overrides()?;

    for (name, template) in config_file.templates.iter() {
        if !overrides.templates.contains_key(name) {
            continue;
        }
        let Some(matugen_core::template::OutputPath::Single(out_path)) = &template.output_path
        else {
            continue;
        };
        let out_abs = expand_tilde(out_path);
        if !out_abs.exists() {
            continue;
        }

        let rendered = match fs::read_to_string(&out_abs) {
            Ok(rendered) => rendered,
            Err(_) => continue,
        };
        let patched = apply_template_overrides(name, &rendered, &overrides);
        let _ = fs::write(out_abs, patched);
    }

    Ok(())
}

#[tauri::command]
pub async fn apply_theme(context: serde_json::Value) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || apply_theme_blocking(context))
        .await
        .map_err(|e| e.to_string())?
}

pub fn apply_theme_blocking(context: serde_json::Value) -> Result<(), String> {
    let Some((config_file, config_path)) = config_file_and_path()? else {
        return Ok(());
    };
    let overrides = read_template_overrides()?;

    for (name, template) in config_file.templates.iter() {
        let input_abs = resolve_template_input(&config_path, &template.input_path);

        if !input_abs.exists() {
            continue;
        }

        let source = match fs::read_to_string(&input_abs) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let result = match render_template(name, source, context.clone()) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let result = apply_template_overrides(name, &result, &overrides);

        if let Some(matugen_core::template::OutputPath::Single(out_path)) = &template.output_path {
            let out_abs = expand_tilde(out_path);

            if let Some(parent) = out_abs.parent() {
                fs::create_dir_all(parent).ok();
            }

            if fs::write(&out_abs, result).is_ok() {
                if let Some(hook) = &template.post_hook {
                    // Bounded: an installed template's post_hook is arbitrary
                    // user/third-party shell (e.g. a Spicetify watcher that
                    // never sees Spotify reload) and must never be able to
                    // stall the whole "apply colors" flow.
                    proc::run_ignoring_result(hook, POST_HOOK_TIMEOUT);
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_installed_templates() -> Result<Vec<String>, String> {
    get_installed_templates_in(&matugen_config_dir()?)
}

fn get_installed_templates_in(config_dir: &Path) -> Result<Vec<String>, String> {
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(config_path).map_err(|e| e.to_string())?;
    let document = template_table(&content)?;
    Ok(document
        .get("templates")
        .and_then(toml::Value::as_table)
        .map(|entries| entries.keys().cloned().collect())
        .unwrap_or_default())
}

#[tauri::command]
pub fn get_installed_template_entries() -> Result<Vec<InstalledTemplateEntry>, String> {
    get_installed_template_entries_in(&matugen_config_dir()?, &CatalogPaths::system()?)
}

fn get_installed_template_entries_in(
    config_dir: &Path,
    paths: &CatalogPaths,
) -> Result<Vec<InstalledTemplateEntry>, String> {
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(config_path).map_err(|e| e.to_string())?;
    let document = template_table(&content)?;
    Ok(document
        .get("templates")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(|entries| entries.iter())
        .filter_map(|(key, entry)| {
            let input = entry.get("input_path")?.as_str()?;
            let output = entry.get("output_path")?.as_str()?;
            let input = portable_output(input, paths).ok()?;
            let output = portable_output(output, paths).ok()?;
            Some(InstalledTemplateEntry {
                key: key.clone(),
                input_path: normalized_output(&input, config_dir)
                    .to_string_lossy()
                    .into_owned(),
                output_path: normalized_output(&output, config_dir)
                    .to_string_lossy()
                    .into_owned(),
            })
        })
        .collect())
}

#[tauri::command]
pub fn uninstall_template(template_name: String) -> Result<(), String> {
    let _guard = TEMPLATE_STORAGE_LOCK.lock().map_err(|e| e.to_string())?;
    uninstall_template_in(&matugen_config_dir()?, &template_name)
}

fn uninstall_template_in(config_dir: &Path, template_name: &str) -> Result<(), String> {
    let safe_name = safe_template_name(template_name)?;
    let name_key = installed_key(safe_name);
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        return Ok(());
    }
    let config_content = fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
    let document = template_table(&config_content)?;
    let Some(entries) = document.get("templates").and_then(toml::Value::as_table) else {
        return Ok(());
    };
    let Some(entry) = entries.get(&name_key) else {
        return Ok(());
    };
    let owned_input = config_dir.join("templates").join(safe_name);
    let remove_owned_input = entry
        .get("input_path")
        .and_then(toml::Value::as_str)
        .is_some_and(|path| {
            normalized_output(path, config_dir)
                == normalized_output(&owned_input.to_string_lossy(), config_dir)
                && !entries.iter().any(|(key, other)| {
                    key != &name_key
                        && other
                            .get("input_path")
                            .and_then(toml::Value::as_str)
                            .is_some_and(|other_path| {
                                normalized_output(other_path, config_dir)
                                    == normalized_output(&owned_input.to_string_lossy(), config_dir)
                            })
                })
        });
    let quoted = format!("[templates.\"{}\"]", escape_toml_string(&name_key));
    let bare = format!("[templates.{name_key}]");
    let mut new_content = String::new();
    let mut skip = false;
    let mut found = false;
    for line in config_content.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            skip = trimmed == quoted || trimmed == bare;
            found |= skip;
        }
        if !skip {
            new_content.push_str(line);
        }
    }
    if !found {
        return Err("Installed template has no removable config section".into());
    }
    atomic_write_text(&config_path, &new_content)?;
    if remove_owned_input && owned_input.exists() {
        fs::remove_file(owned_input).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn sanitize_template_key(template_name: &str) -> String {
    template_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled_templates() -> Vec<TemplateInfo> {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("matugen-themes");
        list_templates_from_themes_dir(dir).expect("bundled templates directory must be readable")
    }

    #[test]
    fn windows_paths_are_valid_inside_toml_strings() {
        let path = r#"C:\Windows\Temp\matugen_windows_term.json"#;
        let document = format!("output_path = \"{}\"", escape_toml_string(path));
        let parsed: toml::Value = toml::from_str(&document).unwrap();
        assert_eq!(parsed["output_path"].as_str(), Some(path));
    }

    /// The app targets KDE Plasma only: tiling-Wayland-compositor tooling
    /// (Hyprland, Niri, Sway, Cosmic, GNOME Shell, ...) must never resurface
    /// in the bundled catalog, even if a future upstream sync re-adds the
    /// files under a new name.
    #[test]
    fn bundled_catalog_has_no_leftover_non_kde_wm_templates() {
        let templates = bundled_templates();
        assert!(!templates.is_empty());

        let banned_needles = [
            "hyprland",
            "hyprwat",
            "niri",
            "sway",
            "labwc",
            "mango",
            "cosmic",
            "gnome-shell",
            "rofi",
            "fuzzel",
            "waybar",
            "wofi",
            "swaync",
            "mako",
            "quickshell",
            "dunst",
            "clipse",
            "windows_term",
        ];
        for template in &templates {
            let haystack = format!(
                "{} {} {}",
                template.relative_path.to_lowercase(),
                template.display_name.to_lowercase(),
                template.target_app.to_lowercase()
            );
            for needle in banned_needles {
                assert!(
                    !haystack.contains(needle),
                    "template '{}' should have been filtered out (matched '{}')",
                    template.relative_path,
                    needle
                );
            }
        }
    }

    #[test]
    fn bundled_catalog_includes_newly_added_kde_relevant_templates() {
        let templates = bundled_templates();
        let names: Vec<&str> = templates.iter().map(|t| t.relative_path.as_str()).collect();
        for expected in [
            "aerc",
            "foot-colors.ini",
            "ghostwriter.json",
            "system24.css",
            "vscode-colors",
            "vscode-colors.json",
            "zen-userchrome.css",
            "zen-usercontent.css",
            "papirus-color",
        ] {
            assert!(
                names.contains(&expected),
                "missing newly added template: {expected}"
            );
        }
    }

    #[test]
    fn every_bundled_template_has_a_curated_category() {
        let templates = bundled_templates();
        let allowed = [
            "KDE Plasma",
            "Terminals",
            "Shell Tools",
            "Browsers",
            "Editors",
            "Apps",
            "Websites",
        ];
        for template in &templates {
            assert!(
                allowed.contains(&template.category.as_str()),
                "template '{}' has unexpected category '{}'",
                template.relative_path,
                template.category
            );
        }
    }

    /// Manual-step text must be localizable: every entry is required to be a
    /// stable `templates.steps.<key>` i18n lookup key (translated client-side
    /// per the app's active language) rather than a hardcoded English
    /// sentence baked into the backend.
    #[test]
    fn manual_steps_are_i18n_keys_not_literal_english_text() {
        let templates = bundled_templates();
        for template in &templates {
            for step in &template.manual_steps {
                assert!(
                    step.starts_with("templates.steps."),
                    "template '{}' has a manual step that isn't an i18n key: '{}'",
                    template.relative_path,
                    step
                );
            }
        }
    }

    struct TempFixture {
        path: PathBuf,
    }

    impl TempFixture {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "matugen_fixture_{}_{}_{}",
                name,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn test_context() -> serde_json::Value {
        let path = std::env::temp_dir().join(format!(
            "template-test-seed-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let image = image::RgbImage::from_fn(112, 112, |x, _| match x / 28 {
            0 => image::Rgb([220, 50, 40]),
            1 => image::Rgb([40, 170, 70]),
            2 => image::Rgb([30, 70, 220]),
            _ => image::Rgb([240, 170, 30]),
        });
        image.save(&path).unwrap();
        let ctx = crate::commands::color::generate_scheme_from_image_blocking(
            path.to_string_lossy().to_string(),
            "Tinted Smart".into(),
            Default::default(),
        )
        .expect("must generate valid test scheme context");
        let _ = fs::remove_file(&path);
        ctx
    }

    #[test]
    fn heroic_detection_isolated_fixtures() {
        let fixture = TempFixture::new("heroic_det");
        let config_dir = fixture.path.join(".config");
        let home_dir = fixture.path.join("home");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&home_dir).unwrap();

        // 1. Neither installed
        let res = detect_heroic_installation_in(&config_dir, &home_dir);
        assert!(!res.native_detected);
        assert!(!res.flatpak_detected);
        assert_eq!(
            res.flatpak_path,
            "~/.var/app/com.heroicgameslauncher.hgl/config/heroic/themes/matugen.css"
        );

        // 2. Only Native installed
        let native_dir = config_dir.join("heroic");
        fs::create_dir_all(&native_dir).unwrap();
        let res = detect_heroic_installation_in(&config_dir, &home_dir);
        assert!(res.native_detected);
        assert!(!res.flatpak_detected);

        // 3. Both installed
        let flatpak_dir = home_dir.join(".var/app/com.heroicgameslauncher.hgl");
        fs::create_dir_all(&flatpak_dir).unwrap();
        let res = detect_heroic_installation_in(&config_dir, &home_dir);
        assert!(res.native_detected);
        assert!(res.flatpak_detected);

        // 4. Only Flatpak installed
        fs::remove_dir_all(&native_dir).unwrap();
        let res = detect_heroic_installation_in(&config_dir, &home_dir);
        assert!(!res.native_detected);
        assert!(res.flatpak_detected);
    }

    #[test]
    fn heroic_metadata_has_deterministic_default() {
        let templates = bundled_templates();
        let heroic = templates
            .iter()
            .find(|t| t.relative_path == "heroic.css")
            .expect("heroic.css must exist in bundled templates");

        assert_eq!(heroic.target_app, "Heroic Games Launcher");
        assert_eq!(heroic.category, "Apps");
        assert_eq!(heroic.automation_level, "config-patch");
        assert_eq!(
            heroic.default_output_path.as_deref(),
            Some("~/.config/heroic/themes/matugen.css")
        );
        assert_eq!(
            heroic.manual_steps,
            vec![
                "templates.steps.heroicCustomThemesPath".to_string(),
                "templates.steps.heroicSelectTheme".to_string(),
            ]
        );
    }

    #[test]
    fn heroic_template_renders_valid_css_without_raw_tokens() {
        let templates = bundled_templates();
        let heroic = templates
            .iter()
            .find(|t| t.relative_path == "heroic.css")
            .unwrap();
        let source = fs::read_to_string(&heroic.path).unwrap();
        let rendered = render_template("heroic", source, test_context()).unwrap();

        assert!(
            !rendered.contains("{{"),
            "must not have unrendered {{ tokens"
        );
        assert!(
            !rendered.contains("}}"),
            "must not have unrendered }} tokens"
        );
        assert!(rendered.contains("--accent: #"));
        assert!(rendered.contains("--primary: #"));
        assert!(rendered.contains("--neutral-06: #"));
        assert!(rendered.contains("--gamecard-title-color: #"));
        assert!(rendered.contains("--secondary-button: var(--accent);"));
    }

    #[test]
    fn install_template_rejects_empty_output_path() {
        let err_empty = install_template(
            "/tmp/some-template".into(),
            "heroic.css".into(),
            "".into(),
            None,
        )
        .unwrap_err();
        assert!(err_empty.contains("Output path cannot be empty"));

        let err_spaces = install_template(
            "/tmp/some-template".into(),
            "heroic.css".into(),
            "    ".into(),
            None,
        )
        .unwrap_err();
        assert!(err_spaces.contains("Output path cannot be empty"));
    }

    #[test]
    fn foot_template_metadata_and_rendering() {
        let templates = bundled_templates();
        let foot = templates
            .iter()
            .find(|t| t.relative_path == "foot-colors.ini")
            .expect("foot-colors.ini must exist in bundled catalog");

        assert_eq!(foot.category, "Terminals");
        assert_eq!(foot.target_app, "Foot");
        assert_eq!(foot.display_name, "Foot");
        assert_eq!(foot.automation_level, "config-patch");
        assert_eq!(
            foot.default_output_path.as_deref(),
            Some("~/.config/foot/foot-colors.ini")
        );
        assert_eq!(
            foot.default_post_hook.as_deref(),
            Some("pkill -SIGUSR1 foot || true")
        );
        assert_eq!(
            foot.manual_steps,
            vec!["templates.steps.footIncludeConfig".to_string()]
        );

        let source = fs::read_to_string(&foot.path).unwrap();
        let rendered = render_template("foot", source, test_context()).unwrap();
        assert!(!rendered.contains("{{"));
        assert!(!rendered.contains("}}"));
        assert!(rendered.contains("[colors-dark]"));
        assert!(rendered.contains("regular0=4c4c4c"));
        assert!(rendered.contains("foreground="));
    }

    #[test]
    fn ghostwriter_template_metadata_and_valid_json() {
        let templates = bundled_templates();
        let gw = templates
            .iter()
            .find(|t| t.relative_path == "ghostwriter.json")
            .expect("ghostwriter.json must exist in bundled catalog");

        assert_eq!(gw.category, "Editors");
        assert_eq!(gw.target_app, "Ghostwriter");
        assert_eq!(gw.display_name, "Ghostwriter");
        assert_eq!(gw.automation_level, "config-patch");
        assert_eq!(
            gw.default_output_path.as_deref(),
            Some("~/.local/share/ghostwriter/themes/Matugen.json")
        );
        assert_eq!(
            gw.manual_steps,
            vec!["templates.steps.ghostwriterSetTheme".to_string()]
        );

        let source = fs::read_to_string(&gw.path).unwrap();
        let rendered = render_template("ghostwriter", source, test_context()).unwrap();
        assert!(!rendered.contains("{{"));
        assert!(!rendered.contains("}}"));

        let parsed: serde_json::Value = serde_json::from_str(&rendered)
            .expect("rendered Ghostwriter template must be valid JSON");
        assert!(parsed["dark"]["accent"].is_string());
        assert!(parsed["light"]["accent"].is_string());
        assert!(parsed["dark"]["background"].is_string());
    }

    #[test]
    fn kitty_template_renders_without_raw_tokens() {
        let templates = bundled_templates();
        let kitty = templates
            .iter()
            .find(|t| t.relative_path == "kitty-colors.conf")
            .expect("kitty-colors.conf must exist");
        let source = fs::read_to_string(&kitty.path).unwrap();
        let rendered = render_template("kitty", source, test_context()).unwrap();

        assert!(!rendered.contains("{{"));
        assert!(!rendered.contains("}}"));
        assert!(rendered.contains("background            #"));
        assert!(rendered.contains("color255              #"));
    }

    #[test]
    fn yazi_template_has_url_syntax_and_no_stale_name_rules() {
        let templates = bundled_templates();
        let yazi = templates
            .iter()
            .find(|t| t.relative_path == "yazi-theme.toml")
            .expect("yazi-theme.toml must exist");
        let source = fs::read_to_string(&yazi.path).unwrap();

        assert!(source.contains(r#"{ url = "*", is = "orphan""#));
        assert!(source.contains(r#"{ url = "*", is = "exec""#));
        assert!(!source.contains(r#"{ name = "*", is = "orphan""#));
        assert!(!source.contains(r#"{ name = "*", is = "exec""#));
    }

    #[test]
    fn youtube_template_has_modern_selectors() {
        let templates = bundled_templates();
        let yt = templates
            .iter()
            .find(|t| t.relative_path == "websites/youtube.css")
            .expect("youtube.css must exist");
        let source = fs::read_to_string(&yt.path).unwrap();

        assert!(source.contains(":root, [dark], [light]"));
        assert!(source.contains("#background.ytd-masthead"));
    }

    fn fixture_paths(root: &Path) -> CatalogPaths {
        CatalogPaths {
            home: root.join("home"),
            config: root.join("xdg/config"),
            data: root.join("xdg/data"),
            cache: root.join("xdg/cache"),
            state: root.join("xdg/state"),
        }
    }

    #[test]
    fn catalog_groups_all_bundled_inputs_once_with_distinct_targets() {
        let fixture = TempFixture::new("catalog");
        let paths = fixture_paths(&fixture.path);
        fs::create_dir_all(paths.config.join("heroic")).unwrap();
        fs::create_dir_all(paths.home.join(".var/app/com.heroicgameslauncher.hgl")).unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/matugen-themes");
        let catalog = list_template_catalog_from_dir(root, &paths).unwrap();
        let app_ids: HashSet<_> = catalog.iter().map(|app| &app.id).collect();
        assert_eq!(app_ids.len(), catalog.len(), "one card per application");
        let mut inputs = HashSet::new();
        for app in &catalog {
            let variants: HashSet<_> = app.variants.iter().map(|variant| &variant.id).collect();
            assert_eq!(variants.len(), app.variants.len());
            for variant in &app.variants {
                let target_ids: HashSet<_> =
                    variant.targets.iter().map(|target| &target.id).collect();
                assert_eq!(
                    target_ids.len(),
                    variant.targets.len(),
                    "duplicate targets in {}",
                    variant.id
                );
                if variant.source.kind == "community" {
                    assert!(variant
                        .source
                        .repository
                        .as_deref()
                        .is_some_and(|s| !s.is_empty()));
                    assert!(variant
                        .source
                        .author
                        .as_deref()
                        .is_some_and(|s| !s.is_empty()));
                    assert!(variant
                        .source
                        .attribution
                        .as_deref()
                        .is_some_and(|s| !s.is_empty()));
                    assert!(variant
                        .source
                        .license
                        .as_deref()
                        .is_some_and(|s| !s.is_empty()));
                    assert!(variant
                        .source
                        .license_status
                        .as_deref()
                        .is_some_and(|s| !s.is_empty()));
                }
                for target in &variant.targets {
                    assert!(
                        !target.output_path.is_empty(),
                        "empty output for {}",
                        variant.id
                    );
                    assert!(target.input_path.starts_with(
                        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("resources/matugen-themes")
                            .to_string_lossy()
                            .as_ref()
                    ));
                    inputs.insert(target.input_path.as_str());
                }
            }
        }
        for template in bundled_templates() {
            assert!(
                inputs.contains(template.path.as_str()),
                "missing {}",
                template.relative_path
            );
        }
        let discord = catalog.iter().find(|app| app.id == "discord").unwrap();
        assert_eq!(discord.variants.len(), 3);
        let filenames: HashSet<_> = discord
            .variants
            .iter()
            .map(|variant| variant.targets[0].output_path.rsplit('/').next().unwrap())
            .collect();
        assert_eq!(
            filenames.len(),
            3,
            "variants must never render over one another"
        );
        let material = discord
            .variants
            .iter()
            .find(|variant| variant.id == "discord.material")
            .unwrap();
        assert_eq!(material.targets.len(), 7);
        assert_eq!(material.source.kind, "community");
        assert_eq!(material.source.license.as_deref(), Some("GPL-2.0-or-later"));
        assert_eq!(material.template.automation_level, "manual");
        assert_eq!(
            material.template.manual_steps,
            vec!["templates.steps.discordActivate"]
        );
        let heroic = catalog.iter().find(|app| app.id == "heroic").unwrap();
        assert_eq!(heroic.variants.len(), 1);
        assert!(heroic.variants[0]
            .targets
            .iter()
            .all(|target| target.detected));
        assert_ne!(
            heroic.variants[0].targets[0].output_path,
            heroic.variants[0].targets[1].output_path
        );
        assert!(heroic.variants[0].targets[0]
            .output_path
            .starts_with(paths.config.to_string_lossy().as_ref()));
        let vscode = catalog.iter().find(|app| app.id == "vscode").unwrap();
        assert_eq!(vscode.variants.len(), 2);
        let premium = vscode
            .variants
            .iter()
            .find(|v| v.id == "vscode.material-premium")
            .unwrap();
        assert_eq!(premium.targets[0].install_type, "manual");
        assert_eq!(premium.template.automation_level, "manual");
        assert_eq!(
            premium.template.manual_steps,
            vec!["templates.steps.vscodeMergeColors"]
        );
    }

    #[test]
    fn original_discord_adapter_renders_both_modes_and_real_hsl() {
        let source = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources/matugen-themes/templates/discord-material.css"),
        )
        .unwrap();
        let rendered = render_template("discord_material", source, test_context()).unwrap();
        assert!(rendered.contains(".theme-dark {"));
        assert!(rendered.contains(".theme-light {"));
        assert!(!rendered.contains("{{") && !rendered.contains("}}"));
        let hsl = Regex::new(r"--accent-hue: [0-9]+(?:\.[0-9]+)?;\s+--accent-saturation: [0-9]+(?:\.[0-9]+)?%;\s+--accent-lightness: [0-9]+(?:\.[0-9]+)?%;").unwrap();
        assert_eq!(hsl.find_iter(&rendered).count(), 2);
    }

    #[test]
    fn install_keys_do_not_overwrite_inputs_or_conflicting_outputs() {
        let fixture = TempFixture::new("install_keys");
        let paths = fixture_paths(&fixture.path);
        let config = fixture.path.join("matugen");
        let source = fixture.path.join("original.css");
        let changed = fixture.path.join("changed.css");
        fs::write(&source, "original").unwrap();
        fs::write(&changed, "changed").unwrap();
        let source = source.to_string_lossy();
        let changed = changed.to_string_lossy();
        let output = "$XDG_CONFIG_HOME/vesktop/themes/matugen-material.css";
        let first = "discord.material__vesktop-native";
        install_template_in(&config, &source, first, output, None, &paths).unwrap();
        let input = config.join("templates").join(first);
        let before = fs::read_to_string(config.join("config.toml")).unwrap();
        assert!(
            install_template_in(&config, &changed, first, output, None, &paths)
                .unwrap_err()
                .contains("already installed")
        );
        assert_eq!(fs::read_to_string(&input).unwrap(), "original");
        assert_eq!(
            fs::read_to_string(config.join("config.toml")).unwrap(),
            before
        );
        let second = "discord-system24__vesktop-native";
        assert!(install_template_in(
            &config,
            &changed,
            second,
            &paths
                .config
                .join("vesktop/themes/matugen-material.css")
                .to_string_lossy(),
            None,
            &paths
        )
        .unwrap_err()
        .contains("already used"));
        assert!(!config.join("templates").join(second).exists());
        install_template_in(
            &config,
            &changed,
            second,
            "$XDG_CONFIG_HOME/vesktop/themes/system24.css",
            None,
            &paths,
        )
        .unwrap();
        let keys = get_installed_templates_in(&config).unwrap();
        assert!(keys.contains(&first.to_string()) && keys.contains(&second.to_string()));
        let parsed: ConfigFile =
            toml::from_str(&fs::read_to_string(config.join("config.toml")).unwrap()).unwrap();
        assert!(parsed.templates.contains_key(first) && parsed.templates.contains_key(second));
        let rendered = paths.config.join("vesktop/themes/system24.css");
        fs::create_dir_all(rendered.parent().unwrap()).unwrap();
        fs::write(&rendered, "keep output").unwrap();
        uninstall_template_in(&config, first).unwrap();
        assert!(!input.exists());
        assert!(config.join("templates").join(second).exists());
        assert_eq!(fs::read_to_string(&rendered).unwrap(), "keep output");
        assert_eq!(
            get_installed_templates_in(&config).unwrap(),
            vec![second.to_string()]
        );
    }

    #[test]
    fn equibop_material_install_and_removal_leave_other_variants_intact() {
        let fixture = TempFixture::new("equibop_material");
        let paths = fixture_paths(&fixture.path);
        let config = fixture.path.join("matugen");
        let themes = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/matugen-themes");
        let catalog = list_template_catalog_from_dir(themes, &paths).unwrap();
        let discord = catalog.iter().find(|app| app.id == "discord").unwrap();

        for variant in &discord.variants {
            let target = variant
                .targets
                .iter()
                .find(|target| target.id == "equibop-native")
                .unwrap();
            let key = format!("{}__{}", variant.id, target.id);
            install_template_in(
                &config,
                &target.input_path,
                &key,
                &target.output_path,
                None,
                &paths,
            )
            .unwrap();
        }
        let material = discord
            .variants
            .iter()
            .find(|variant| variant.id == "discord.material")
            .unwrap();
        let flatpak = material
            .targets
            .iter()
            .find(|target| target.id == "equibop-flatpak")
            .unwrap();
        let flatpak_key = format!("{}__{}", material.id, flatpak.id);
        install_template_in(
            &config,
            &flatpak.input_path,
            &flatpak_key,
            &flatpak.output_path,
            None,
            &paths,
        )
        .unwrap();
        let material_key = "discord.material__equibop-native";
        assert_eq!(get_installed_templates_in(&config).unwrap().len(), 4);
        assert_eq!(
            get_installed_template_entries_in(&config, &paths)
                .unwrap()
                .len(),
            4
        );

        uninstall_template_in(&config, material_key).unwrap();
        let remaining = get_installed_templates_in(&config).unwrap();
        assert_eq!(remaining.len(), 3);
        assert!(!remaining.contains(&material_key.to_string()));
        assert!(remaining.contains(&flatpak_key));
        assert!(remaining.contains(&"discord-midnight__equibop-native".to_string()));
        assert!(remaining.contains(&"discord-system24__equibop-native".to_string()));
        assert!(!config.join("templates").join(material_key).exists());
        assert!(config.join("templates").join(&flatpak_key).exists());
    }

    #[test]
    fn unmanaged_output_is_never_overwritten_by_install() {
        let fixture = TempFixture::new("unmanaged_output");
        let paths = fixture_paths(&fixture.path);
        let config = fixture.path.join("matugen");
        fs::create_dir_all(&config).unwrap();
        let config_path = config.join("config.toml");
        fs::write(&config_path, "[config]\n# user settings\n").unwrap();
        let source = fixture.path.join("original.css");
        fs::write(&source, "template").unwrap();
        let output = paths.config.join("vesktop/themes/matugen-material.css");
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(&output, "existing user theme").unwrap();
        let error = install_template_in(
            &config,
            &source.to_string_lossy(),
            "discord.material__vesktop-native",
            "$XDG_CONFIG_HOME/vesktop/themes/matugen-material.css",
            None,
            &paths,
        )
        .unwrap_err();
        assert!(error.contains("already exists"));
        assert_eq!(fs::read_to_string(&output).unwrap(), "existing user theme");
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "[config]\n# user settings\n"
        );
        assert!(!config
            .join("templates/discord.material__vesktop-native")
            .exists());
        let relative_output = config.join("already.css");
        fs::write(&relative_output, "user relative output").unwrap();
        assert!(install_template_in(
            &config,
            &source.to_string_lossy(),
            "other__manual",
            "already.css",
            None,
            &paths,
        )
        .unwrap_err()
        .contains("already exists"));
        assert_eq!(
            fs::read_to_string(relative_output).unwrap(),
            "user relative output"
        );
        install_template_in(
            &config,
            &source.to_string_lossy(),
            "safe__manual",
            "new-theme.css",
            None,
            &paths,
        )
        .unwrap();
        let saved = template_table(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(
            saved["templates"]["safe__manual"]["output_path"].as_str(),
            Some(config.join("new-theme.css").to_string_lossy().as_ref())
        );
        let mut existing_config = fs::read_to_string(&config_path).unwrap();
        existing_config.push_str(
            "\n[templates.multi]\ninput_path = \"templates/other.css\"\noutput_path = [\"/tmp/unused-output.css\", \"$XDG_CACHE_HOME/matugen/shared.css\"]\n",
        );
        fs::write(&config_path, &existing_config).unwrap();
        assert!(install_template_in(
            &config,
            &source.to_string_lossy(),
            "other__cache",
            "$XDG_CACHE_HOME/matugen/shared.css",
            None,
            &paths,
        )
        .unwrap_err()
        .contains("already used"));
        assert_eq!(fs::read_to_string(&config_path).unwrap(), existing_config);
        assert!(!config.join("templates/other__cache").exists());
    }

    #[test]
    fn installed_entries_resolve_legacy_and_pair_target_paths() {
        let fixture = TempFixture::new("installed_entries");
        let paths = fixture_paths(&fixture.path);
        let config = fixture.path.join("matugen");
        fs::create_dir_all(&config).unwrap();
        fs::write(
            config.join("config.toml"),
            "[config]\n[templates.heroic_css]\ninput_path = \"templates/heroic.css\"\noutput_path = \"~/.var/app/com.heroicgameslauncher.hgl/config/heroic/themes/matugen.css\"\n[templates.vscode_colors_json]\ninput_path = \"templates/vscode-colors.json\"\noutput_path = \"$XDG_CACHE_HOME/matugen/vscode-colors.json\"\n[templates.\"discord.material__vesktop-native\"]\ninput_path = \"templates/discord.material__vesktop-native\"\noutput_path = \"$XDG_CONFIG_HOME/vesktop/themes/matugen-material.css\"\n",
        ).unwrap();
        let entries = get_installed_template_entries_in(&config, &paths).unwrap();
        assert_eq!(entries.len(), 3);
        let heroic = entries
            .iter()
            .find(|entry| entry.key == "heroic_css")
            .unwrap();
        assert_eq!(
            heroic.input_path,
            config.join("templates/heroic.css").to_string_lossy()
        );
        assert_eq!(
            heroic.output_path,
            paths
                .home
                .join(".var/app/com.heroicgameslauncher.hgl/config/heroic/themes/matugen.css")
                .to_string_lossy()
        );
        let vscode = entries
            .iter()
            .find(|entry| entry.key == "vscode_colors_json")
            .unwrap();
        assert_eq!(
            vscode.output_path,
            paths
                .cache
                .join("matugen/vscode-colors.json")
                .to_string_lossy()
        );
        let discord = entries
            .iter()
            .find(|entry| entry.key == "discord.material__vesktop-native")
            .unwrap();
        assert_eq!(
            discord.output_path,
            paths
                .config
                .join("vesktop/themes/matugen-material.css")
                .to_string_lossy()
        );
    }

    #[test]
    fn legacy_uninstall_preserves_shared_or_external_inputs() {
        let fixture = TempFixture::new("legacy_install");
        let paths = fixture_paths(&fixture.path);
        let config = fixture.path.join("matugen");
        let source = fixture.path.join("legacy.css");
        fs::write(&source, "legacy").unwrap();
        install_template_in(
            &config,
            &source.to_string_lossy(),
            "heroic.css",
            "~/heroic.css",
            None,
            &paths,
        )
        .unwrap();
        let shared = config.join("templates/heroic.css");
        let content = fs::read_to_string(config.join("config.toml")).unwrap();
        fs::write(config.join("config.toml"), format!("{content}\n[templates.other]\ninput_path = \"{}\"\noutput_path = \"/tmp/other.css\"\n", shared.display())).unwrap();
        assert!(get_installed_templates_in(&config)
            .unwrap()
            .contains(&"heroic_css".to_string()));
        uninstall_template_in(&config, "heroic.css").unwrap();
        assert!(shared.exists(), "other entry owns this input too");
        assert!(get_installed_templates_in(&config)
            .unwrap()
            .contains(&"other".to_string()));
        let content = fs::read_to_string(config.join("config.toml")).unwrap();
        fs::write(config.join("config.toml"), format!("{content}\n[templates.\"external__manual\"]\ninput_path = \"{}\"\noutput_path = \"/tmp/external.css\"\n", source.display())).unwrap();
        uninstall_template_in(&config, "external__manual").unwrap();
        assert!(source.exists(), "external source is not managed input");
    }
    #[test]
    fn portable_xdg_output_is_concrete_and_rejects_escape() {
        let fixture = TempFixture::new("xdg_paths");
        let paths = fixture_paths(&fixture.path);
        for (variable, root) in [
            ("$XDG_CONFIG_HOME", &paths.config),
            ("$XDG_DATA_HOME", &paths.data),
            ("$XDG_CACHE_HOME", &paths.cache),
            ("$XDG_STATE_HOME", &paths.state),
        ] {
            assert_eq!(
                portable_output(&format!("{variable}/themes/matugen.css"), &paths).unwrap(),
                root.join("themes/matugen.css").to_string_lossy()
            );
            assert!(portable_output(&format!("{variable}/../escape"), &paths).is_err());
        }
        let config = fixture.path.join("matugen");
        let source = fixture.path.join("source.css");
        fs::write(&source, "palette").unwrap();
        install_template_in(
            &config,
            &source.to_string_lossy(),
            "portable__cache",
            "$XDG_CACHE_HOME/app/theme.css",
            None,
            &paths,
        )
        .unwrap();
        let document =
            template_table(&fs::read_to_string(config.join("config.toml")).unwrap()).unwrap();
        assert_eq!(
            document["templates"]["portable__cache"]["output_path"].as_str(),
            Some(paths.cache.join("app/theme.css").to_string_lossy().as_ref())
        );
    }

    #[test]
    fn original_vscode_fragment_is_mergeable_json() {
        let source = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources/matugen-themes/templates/vscode-material-premium.json"),
        )
        .unwrap();
        let rendered = render_template("vscode_material_premium", source, test_context()).unwrap();
        let document: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        let object = document.as_object().unwrap();
        assert_eq!(object.len(), 2);
        assert!(object["workbench.colorCustomizations"].is_object());
        assert!(object["editor.tokenColorCustomizations"].is_object());
        assert!(!rendered.contains("{{") && !rendered.contains("}}"));
    }
}
