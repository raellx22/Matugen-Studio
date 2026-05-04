use execute::{shell, Execute};
use matugen_core::parser::Engine;
use matugen_core::util::config::ConfigFile;
use matugen_core::State;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::Manager;

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
        || relative.ends_with("/cosmic_postprocess.py")
        || relative.ends_with("/windows_term_post.ps1")
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
        category: "Applications".to_string(),
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
            "Enable legacy user profile stylesheets in Firefox-based browsers.".to_string(),
            "Copy website CSS files into the selected profile chrome/websites directory."
                .to_string(),
            "Import the generated colors.css and website CSS files from UserContent.css."
                .to_string(),
        ];
        return meta;
    }

    match file_name {
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
            meta.manual_steps = vec![
                "Matugen Studio can generate both Kvantum files; selecting the Kvantum engine may still depend on your Plasma setup.".to_string(),
            ];
        }
        "qtct-colors.conf" => {
            meta.category = "KDE Plasma".to_string();
            meta.target_app = "Qt qt5ct/qt6ct".to_string();
            meta.automation_level = "config-patch".to_string();
            meta.required_commands = vec!["qt5ct".to_string(), "qt6ct".to_string()];
            meta.manual_steps = vec![
                "Set QT_QPA_PLATFORMTHEME to qt6ct when using the qtct path.".to_string(),
                "Qt style packages such as Breeze or Darkly must be installed by the user."
                    .to_string(),
            ];
        }
        "gtk-colors.css" => {
            meta.category = "KDE Plasma".to_string();
            meta.target_app = "GTK 3/4".to_string();
            meta.automation_level = "config-patch".to_string();
            meta.manual_steps =
                vec!["Import colors.css from gtk.css for GTK 3 and GTK 4.".to_string()];
        }
        "kitty-colors.conf" | "ghostty" | "alacritty.toml" | "wezterm_theme.toml" => {
            meta.category = "Terminals".to_string();
            meta.automation_level = "config-patch".to_string();
        }
        "terminal-sequences" | "tmux-colors.conf" | "zellij-theme.kdl.tera" => {
            meta.category = "Terminals".to_string();
            meta.automation_level = "config-patch".to_string();
        }
        "firefox-colors.css" | "pywalfox-colors.json" | "vivaldi.css" => {
            meta.category = "Browsers".to_string();
            meta.automation_level = "manual".to_string();
        }
        "midnight-discord.css" | "spicetify.ini" | "steam.css" | "telegram.tdesktop-theme" => {
            meta.category = "Apps".to_string();
            meta.automation_level = "manual".to_string();
        }
        "nvim-colors.vim" | "template.lua" | "helix.toml" | "zed-colors.json" | "obsidian.css" => {
            meta.category = "Editors".to_string();
            meta.automation_level = if file_name == "helix.toml" {
                "config-patch".to_string()
            } else {
                "manual".to_string()
            };
        }
        "rofi-colors.rasi" | "fuzzel.ini" | "waybar.css" | "colors.css" | "swaync" => {
            meta.category = "Shell".to_string();
            meta.automation_level = "config-patch".to_string();
        }
        _ => {}
    }

    if relative_path == "neovim/template.lua" {
        meta.display_name = "Neovim Lua".to_string();
        meta.target_app = "Neovim".to_string();
        meta.related_files = vec!["templates/neovim/init.lua".to_string()];
        meta.manual_steps = vec![
            "Install and configure base16-colorscheme if you want the advanced Lua integration."
                .to_string(),
            "Source the generated file from init.lua and register a SIGUSR1 reload hook."
                .to_string(),
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
        "alacritty.toml" => Some("~/.config/alacritty/colors.toml"),
        "btop.theme" => Some("~/.config/btop/themes/matugen.theme"),
        "cava-colors.ini" => Some("~/.config/cava/themes/matugen"),
        "clipse_theme.json" => Some("~/.config/clipse/custom_theme.json"),
        "colors.css" => Some("~/.config/matugen/colors.css"),
        "cosmic_theme.ron" => Some("~/.config/matugen/themes/matugen_cosmic.theme.ron"),
        "dunstrc-colors" => Some("~/.config/dunst/dunstrc"),
        "firefox-colors.css" => Some("~/.cache/matugen/firefox/colors.css"),
        "fuzzel.ini" => Some("~/.config/fuzzel/colors.ini"),
        "ghostty" => Some("~/.config/ghostty/themes/Matugen"),
        "gtk-colors.css" => Some("~/.config/gtk-4.0/colors.css"),
        "helix.toml" => Some("~/.config/helix/themes/matugen.toml"),
        "heroic.css" => Some("~/.config/heroic/themes/matugen.css"),
        "hyprland-colors.conf" => Some("~/.config/hypr/colors.conf"),
        "kitty-colors.conf" => Some("~/.config/kitty/themes/Matugen.conf"),
        "kvantum-colors.kvconfig" => Some("~/.config/Kvantum/matugen/matugen.kvconfig"),
        "kvantum-colors.svg" => Some("~/.config/Kvantum/matugen/matugen.svg"),
        "labwc" => Some("~/.config/labwc/themerc-override"),
        "mako" => Some("~/.config/mako/mako-colors"),
        "mango.conf" => Some("~/.config/mango/colors.conf"),
        "matugen.obt" => Some("~/.config/obs-studio/themes/matugen.obt"),
        "mcfly.toml" => Some("~/.local/share/mcfly/config.toml"),
        "micro.micro" => Some("~/.config/micro/colorschemes/matugen.micro"),
        "midnight-discord.css" => Some("~/.config/vesktop/themes/midnight-discord.css"),
        "niri-colors.kdl" => Some("~/.config/niri/colors.kdl"),
        "nvim-colors.vim" => Some("~/.config/nvim/colors/matugen.vim"),
        "obsidian.css" => Some("~/Documents/ObsidianVault/.obsidian/snippets/matugen.css"),
        "opencode-colors.json" => Some("~/.config/opencode/themes/matugen.json"),
        "prismlauncher.json" => Some("~/.local/share/PrismLauncher/themes/Matugen/theme.json"),
        "pywalfox-colors.json" => Some("~/.cache/wal/colors.json"),
        "qtct-colors.conf" => Some("~/.config/qt6ct/colors/matugen.conf"),
        "quickshell.json" => Some("~/.local/state/quickshell/generated/colors.json"),
        "quickshell.qml" => Some("~/.config/quickshell/Colors.qml"),
        "rmpc.ron" => Some("~/.config/rmpc/themes/matugen.ron"),
        "rofi-colors.rasi" => Some("~/.config/rofi/colors.rasi"),
        "spicetify.ini" => Some("~/.config/spicetify/Themes/Sleek/color.ini"),
        "starship-colors.toml" => Some("~/.config/starship.toml"),
        "steam.css" => Some("~/.config/AdwSteamGtk/custom.css"),
        "sway-colors.conf" => Some("~/.config/sway/colors.conf"),
        "telegram.tdesktop-theme" => Some("~/Downloads/matugen.tdesktop-theme"),
        "television.toml" => Some("~/.config/television/themes/matugen.toml"),
        "terminal-sequences" => Some("~/.cache/terminal-sequences"),
        "tmux-colors.conf" => Some("~/.config/tmux/generated.conf"),
        "vivaldi.css" => Some("~/.config/vivaldi-matugen/vivaldi.css"),
        "waybar.css" => Some("~/.config/waybar/colors.css"),
        "wezterm_theme.toml" => Some("~/.config/wezterm/colors/matugen_theme.toml"),
        "wine.reg" => Some("/tmp/wine.reg"),
        "windows_term.json" => Some("C:\\Windows\\Temp\\matugen_windows_term.json"),
        "wlogout.css" => Some("~/.config/wlogout/colors.css"),
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
        "dunstrc-colors" => Some("dunstctl reload"),
        "ghostty" => Some("pkill -SIGUSR2 ghostty"),
        "kitty-colors.conf" => Some("kitty +kitten themes --reload-in=all Matugen"),
        "labwc" => Some("labwc -reload"),
        "mako" => Some("makoctl reload"),
        "mango.conf" => Some("mmsg -d reload_config"),
        "niri-colors.kdl" => Some("niri msg action load-config-file"),
        "nvim-colors.vim" | "template.lua" => Some("pkill -SIGUSR1 nvim"),
        "pywalfox-colors.json" => Some("pywalfox update"),
        "spicetify.ini" => Some("spicetify watch -s 2>&1 | sed \"/Reloaded Spotify/q\""),
        "steam.css" => Some("adwaita-steam-gtk -i"),
        "sway-colors.conf" => Some("swaymsg reload"),
        "swaync.css" => Some("swaync-client -rs"),
        "terminal-sequences" => Some("cat ~/.cache/terminal-sequences > /dev/pts/[0-9]*"),
        "tmux-colors.conf" => Some("tmux source-file ~/.config/tmux/generated.conf"),
        "waybar.css" => Some("pkill -SIGUSR2 waybar"),
        "wezterm_theme.toml" => Some("touch ~/.config/wezterm/wezterm.lua"),
        "wine.reg" => Some("wine regedit /tmp/wine.reg"),
        "zellij-theme.kdl.tera" => Some("touch ~/.config/zellij/config.kdl"),
        _ => None,
    }
}

fn manual_steps(file_name: &str) -> Vec<String> {
    match file_name {
        "alacritty.toml" => vec!["Add import = [\"colors.toml\"] to alacritty.toml.".to_string()],
        "btop.theme" => vec!["Choose the matugen theme once from btop settings.".to_string()],
        "cava-colors.ini" => vec!["Set theme = 'matugen' in ~/.config/cava/config.".to_string()],
        "fuzzel.ini" => vec!["Include ~/.config/fuzzel/colors.ini from fuzzel.ini.".to_string()],
        "ghostty" => vec!["Set theme = \"Matugen\" in ~/.config/ghostty/config.".to_string()],
        "helix.toml" => vec!["Set theme = \"matugen\" in ~/.config/helix/config.toml.".to_string()],
        "kitty-colors.conf" => {
            vec!["Apply the Matugen theme once with kitty kitten themes.".to_string()]
        }
        "mako" => vec!["Add include=~/.config/mako/mako-colors to mako config.".to_string()],
        "midnight-discord.css" => {
            vec!["Activate the generated theme from Vencord/Vesktop settings.".to_string()]
        }
        "telegram.tdesktop-theme" => vec![
            "Telegram cannot apply themes automatically.".to_string(),
            "Send the generated .tdesktop-theme file to any chat, open it, then apply it."
                .to_string(),
        ],
        "vivaldi.css" => vec![
            "Enable CSS modifications in vivaldi://experiments.".to_string(),
            "Select the generated CSS folder in Vivaldi appearance settings.".to_string(),
        ],
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
    template_path: String,
    context: serde_json::Value,
) -> Result<String, String> {
    let mut engine = Engine::new();

    // Register filters
    State::add_engine_filters(&mut engine);

    // Read the template file
    let source = fs::read_to_string(&template_path)
        .map_err(|e| format!("Failed to read template: {}", e))?;

    engine.add_template("preview".to_string(), source);
    engine.add_context(context);

    let result = engine.render("preview").map_err(|errs| {
        let mut err_msg = String::new();
        for err in errs {
            err_msg.push_str(&format!("{:?}\n", err));
        }
        err_msg
    })?;

    Ok(result)
}

#[tauri::command]
pub fn install_template(
    template_path: String,
    template_name: String,
    output_path: String,
    post_hook: Option<String>,
) -> Result<(), String> {
    let config_dir = matugen_config_dir()?;

    let templates_dir = config_dir.join("templates");
    fs::create_dir_all(&templates_dir).map_err(|e| e.to_string())?;

    let dest_path = templates_dir.join(&template_name);
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
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
        dest_path.to_string_lossy()
    ));
    config_content.push_str(&format!("output_path = \"{}\"\n", output_path));

    if let Some(hook) = post_hook {
        if !hook.trim().is_empty() {
            config_content.push_str(&format!("post_hook = \"{}\"\n", hook.trim()));
        }
    }

    fs::write(&config_path, config_content).map_err(|e| e.to_string())?;

    Ok(())
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
    let content = serde_json::to_string_pretty(overrides).map_err(|e| e.to_string())?;
    fs::write(config_dir.join("template-overrides.json"), content).map_err(|e| e.to_string())
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
    engine.add_context(context);
    engine.add_template(name.to_string(), source);

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
                TemplateColorControl {
                    key: control.key,
                    name: control.name,
                    description: control.description,
                    source_path: control.source_path,
                    current_hex: override_hex
                        .map(|value| value.hex().to_string())
                        .unwrap_or_else(|| control.original_hex.clone()),
                    original_hex: control.original_hex,
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
    let hex = validate_hex(&hex)?;
    let original_hex = original_hex.as_deref().map(validate_hex).transpose()?;
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
                    let mut cmd = shell(hook);
                    cmd.execute().ok();
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
    let config_dir = matugen_config_dir()?;

    let name_key = sanitize_template_key(&template_name);
    let dest_path = config_dir.join("templates").join(&template_name);

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
        fs::write(&config_path, new_lines.join("\n")).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn sanitize_template_key(template_name: &str) -> String {
    template_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
