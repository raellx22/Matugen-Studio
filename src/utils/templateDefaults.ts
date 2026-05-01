export interface TemplateConfig {
  outputPath: string;
  postHook?: string;
}

export const TEMPLATE_DEFAULTS: Record<string, TemplateConfig> = {
  "alacritty.toml": {
    outputPath: "~/.config/alacritty/colors.toml",
  },
  "btop.theme": {
    outputPath: "~/.config/btop/themes/matugen.theme"
  },
  "kitty-colors.conf": {
    outputPath: "~/.config/kitty/colors.conf",
    postHook: "killall -SIGUSR1 kitty"
  },
  "hyprland-colors.conf": {
    outputPath: "~/.config/hypr/colors.conf",
  },
  "dunstrc-colors": {
    outputPath: "~/.config/dunst/colors",
    postHook: "killall dunst"
  },
  "fuzzel.ini": {
    outputPath: "~/.config/fuzzel/colors.ini"
  },
  "Matugen.colors": {
    outputPath: "~/.local/share/color-schemes/Matugen.colors",
    postHook: "plasma-apply-colorscheme Matugen"
  },
  "cava-colors.ini": {
    outputPath: "~/.config/cava/colors.ini"
  },
  "rofi-colors.rasi": {
    outputPath: "~/.config/rofi/colors.rasi"
  },
  "waybar.css": {
    outputPath: "~/.config/waybar/colors.css",
    postHook: "killall -SIGUSR2 waybar"
  },
  "sway-colors.conf": {
    outputPath: "~/.config/sway/colors.conf",
    postHook: "swaymsg reload"
  },
  "ghostty": {
    outputPath: "~/.config/ghostty/colors"
  },
  "mako": {
    outputPath: "~/.config/mako/colors",
    postHook: "makoctl reload"
  },
  "gtk-colors.css": {
    outputPath: "~/.config/gtk-4.0/colors.css"
  },
  "wezterm_theme.toml": {
    outputPath: "~/.config/wezterm/colors.toml"
  },
  "zed-colors.json": {
    outputPath: "~/.config/zed/themes/matugen.json"
  },
  "yazi-theme.toml": {
    outputPath: "~/.config/yazi/theme.toml"
  },
  "zellij-theme.kdl.tera": {
    outputPath: "~/.config/zellij/themes/matugen.kdl"
  },
  "starship-colors.toml": {
    outputPath: "~/.config/starship/colors.toml"
  },
  "spicetify.ini": {
    outputPath: "~/.config/spicetify/Themes/matugen/color.ini",
    postHook: "spicetify apply"
  },
  "obsidian.css": {
    outputPath: "~/.config/obsidian/themes/matugen.css"
  },
  "heroic.css": {
    outputPath: "~/.config/heroic/themes/matugen.css"
  },
  "midnight-discord.css": {
    outputPath: "~/.config/vesktop/themes/matugen.css"
  },
  "qtct-colors.conf": {
    outputPath: "~/.config/qt5ct/colors/matugen.conf"
  },
  "kvantum-colors.svg": {
    outputPath: "~/.config/Kvantum/matugen/matugen.svg"
  },
  "kvantum-colors.kvconfig": {
    outputPath: "~/.config/Kvantum/matugen/matugen.kvconfig",
    postHook: "kvantummanager --set matugen"
  }
};

export function getDefaultConfig(templateName: string): TemplateConfig {
  if (TEMPLATE_DEFAULTS[templateName]) {
    return TEMPLATE_DEFAULTS[templateName];
  }
  
  // Smart fallback
  const baseName = templateName.replace(/\.[^/.]+$/, "").toLowerCase().replace(/[-_]colors?/, "");
  
  let extension = "conf";
  if (templateName.endsWith(".css")) extension = "css";
  if (templateName.endsWith(".json")) extension = "json";
  if (templateName.endsWith(".toml")) extension = "toml";
  if (templateName.endsWith(".ini")) extension = "ini";
  if (templateName.endsWith(".rasi")) extension = "rasi";
  
  return {
    outputPath: `~/.config/${baseName}/colors.${extension}`
  };
}
