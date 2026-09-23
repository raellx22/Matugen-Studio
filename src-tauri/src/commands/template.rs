use crate::commands::proc;
use matugen_core::parser::Engine;
use matugen_core::util::config::ConfigFile;
use matugen_core::State;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
            meta.automation_level = "manual".to_string();
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
        "ghostty" => Some("~/.config/ghostty/themes/Matugen"),
        "gtk-colors.css" => Some("~/.config/gtk-4.0/colors.css"),
        "helix.toml" => Some("~/.config/helix/themes/matugen.toml"),
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
        "ghostty" => vec!["templates.steps.ghosttySetTheme".to_string()],
        "helix.toml" => vec!["templates.steps.helixSetTheme".to_string()],
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
    let _guard = TEMPLATE_STORAGE_LOCK.lock().map_err(|e| e.to_string())?;
    let config_dir = matugen_config_dir()?;

    let templates_dir = config_dir.join("templates");
    fs::create_dir_all(&templates_dir).map_err(|e| e.to_string())?;
    let safe_name = Path::new(&template_name)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| *name == template_name && !name.contains(".."))
        .ok_or("Invalid template name")?;
    let dest_path = templates_dir.join(safe_name);
    fs::copy(&template_path, &dest_path).map_err(|e| e.to_string())?;

    let config_path = config_dir.join("config.toml");
    let mut config_content = String::new();
    if config_path.exists() {
        config_content = fs::read_to_string(&config_path).unwrap_or_default();
    }

    if !config_content.contains("[config]") {
        config_content = "[config]\n".to_string() + &config_content;
    }

    let name_key = sanitize_template_key(&template_name);

    if config_content.contains(&format!("[templates.{}]", name_key)) {
        return Err(
            "Template already installed. Please check your ~/.config/matugen/config.toml"
                .to_string(),
        );
    }

    config_content.push_str(&format!("\n[templates.{}]\n", name_key));
    config_content.push_str(&format!(
        "input_path = \"{}\"\n",
        escape_toml_string(&dest_path.to_string_lossy())
    ));
    config_content.push_str(&format!(
        "output_path = \"{}\"\n",
        escape_toml_string(&output_path)
    ));

    if let Some(hook) = post_hook {
        if !hook.trim().is_empty() {
            config_content.push_str(&format!(
                "post_hook = \"{}\"\n",
                escape_toml_string(hook.trim())
            ));
        }
    }

    atomic_write_text(&config_path, &config_content)?;

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

fn expand_tilde(path: &PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&s[2..]);
        }
    }
    path.clone()
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
            let out_abs = expand_tilde(&out_path);

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
    let config_dir = matugen_config_dir()?;

    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        return Ok(vec![]);
    }

    let config_content = fs::read_to_string(&config_path).unwrap_or_default();
    let mut installed = Vec::new();

    for line in config_content.lines() {
        let line = line.trim();
        if line.starts_with("[templates.") && line.ends_with(']') {
            let name_key = &line[11..line.len() - 1];
            installed.push(name_key.to_string());
        }
    }

    Ok(installed)
}

#[tauri::command]
pub fn uninstall_template(template_name: String) -> Result<(), String> {
    let _guard = TEMPLATE_STORAGE_LOCK.lock().map_err(|e| e.to_string())?;
    let safe_name = Path::new(&template_name)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| *name == template_name && !name.contains(".."))
        .ok_or("Invalid template name")?;
    let config_dir = matugen_config_dir()?;

    let name_key = sanitize_template_key(safe_name);
    let dest_path = config_dir.join("templates").join(safe_name);

    if dest_path.exists() {
        fs::remove_file(&dest_path).ok();
    }

    let config_path = config_dir.join("config.toml");
    if config_path.exists() {
        let config_content = fs::read_to_string(&config_path).unwrap_or_default();
        let mut new_lines = Vec::new();
        let mut skip = false;

        for line in config_content.lines() {
            let t_line = line.trim();
            if t_line.starts_with("[templates.") {
                skip = t_line == format!("[templates.{}]", name_key);
            } else if t_line.starts_with('[') {
                skip = false;
            }
            if !skip {
                new_lines.push(line);
            }
        }
        atomic_write_text(&config_path, &new_lines.join("\n"))?;
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
}
