export type AppThemeMode = "dark" | "light" | "auto";

export const APP_THEME_STORAGE_KEY = "matugenStudioAppTheme";

type CssVars = Record<string, string>;

export interface AppThemeSnapshot {
  mode: "dark" | "light";
  vars: CssVars;
}

const STATIC_DARK_THEME: AppThemeSnapshot = {
  mode: "dark",
  vars: {
    "--surface-base": "#0f1117",
    "--surface-solid": "#151821",
    "--surface-elevated": "rgba(255, 255, 255, 0.055)",
    "--surface-elevated-hover": "rgba(255, 255, 255, 0.085)",
    "--text-primary": "rgba(255, 255, 255, 0.92)",
    "--text-secondary": "rgba(255, 255, 255, 0.68)",
    "--text-muted": "rgba(255, 255, 255, 0.48)",
    "--border-subtle": "rgba(255, 255, 255, 0.10)",
    "--border-strong": "rgba(255, 255, 255, 0.18)",
    "--accent": "#a8c7fa",
    "--on-accent": "#10131a",
    "--accent-soft": "rgba(168, 199, 250, 0.18)",
    "--danger": "#ffb4ab",
    "--danger-soft": "rgba(255, 180, 171, 0.14)",
    "--app-glow": "rgba(168, 199, 250, 0.08)",
    "--shadow-card": "0 16px 40px rgba(0, 0, 0, 0.22)",
    "--shadow-soft": "0 8px 24px rgba(0, 0, 0, 0.18)",
  },
};

const normalizeHex = (hex?: string | null) => {
  if (!hex) return null;
  const value = hex.trim();
  if (value.length === 9 && value.startsWith("#")) {
    return `#${value.slice(1, 7)}`.toUpperCase();
  }
  if (value.length === 7 && value.startsWith("#")) {
    return value.toUpperCase();
  }
  return null;
};

const hexToRgb = (hex: string) => {
  const stripped = hex.replace("#", "");
  return {
    r: parseInt(stripped.slice(0, 2), 16),
    g: parseInt(stripped.slice(2, 4), 16),
    b: parseInt(stripped.slice(4, 6), 16),
  };
};

const rgba = (hex: string, alpha: number) => {
  const { r, g, b } = hexToRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
};

const readColor = (
  schemeData: any,
  key: string,
  mode: "dark" | "light",
  fallback: string,
) => {
  const color = schemeData?.colors?.[key];
  if (!color) return fallback;

  for (const variant of [mode, "default", mode === "dark" ? "light" : "dark"]) {
    const value = normalizeHex(color[variant]?.hex || color[variant]?.color);
    if (value) return value;
  }

  return fallback;
};

export const resolveAppThemeMode = (mode: AppThemeMode, schemeData: any): "dark" | "light" => {
  if (mode === "dark" || mode === "light") return mode;
  return (schemeData?.wallpaper_luminance ?? schemeData?.source_color_luminance ?? 0) > 0.5 ? "light" : "dark";
};

export const buildAppTheme = (schemeData: any, mode: AppThemeMode): AppThemeSnapshot => {
  if (!schemeData?.colors) return STATIC_DARK_THEME;

  const resolvedMode = resolveAppThemeMode(mode, schemeData);
  const surface = readColor(schemeData, "surface", resolvedMode, STATIC_DARK_THEME.vars["--surface-base"]);
  const surfaceContainer = readColor(schemeData, "surface_container", resolvedMode, STATIC_DARK_THEME.vars["--surface-solid"]);
  const surfaceContainerHigh = readColor(schemeData, "surface_container_high", resolvedMode, surfaceContainer);
  const surfaceContainerLow = readColor(schemeData, "surface_container_low", resolvedMode, surface);
  const surfaceContainerLowest = readColor(schemeData, "surface_container_lowest", resolvedMode, surface);
  const surfaceContainerHighest = readColor(schemeData, "surface_container_highest", resolvedMode, surfaceContainerHigh);
  const onSurface = readColor(schemeData, "on_surface", resolvedMode, resolvedMode === "dark" ? "#FFFFFF" : "#111111");
  const onSurfaceVariant = readColor(schemeData, "on_surface_variant", resolvedMode, onSurface);
  const outlineVariant = readColor(schemeData, "outline_variant", resolvedMode, onSurfaceVariant);
  const secondaryContainer = readColor(schemeData, "secondary_container", resolvedMode, surfaceContainerHigh);
  const primary = readColor(schemeData, "primary", resolvedMode, STATIC_DARK_THEME.vars["--accent"]);
  const onPrimary = readColor(schemeData, "on_primary", resolvedMode, STATIC_DARK_THEME.vars["--on-accent"]);
  const error = readColor(schemeData, "error", resolvedMode, STATIC_DARK_THEME.vars["--danger"]);

  const vars: CssVars = {
    "--surface-base": surface,
    "--surface-solid": surfaceContainerLow,
    "--surface-workspace": surfaceContainerLowest,
    "--surface-section": surfaceContainer,
    "--surface-interactive": surfaceContainerHigh,
    "--surface-interactive-hover": surfaceContainerHighest,
    "--nav-selected": secondaryContainer,
    "--surface-elevated": rgba(onSurface, resolvedMode === "dark" ? 0.055 : 0.065),
    "--surface-elevated-hover": rgba(onSurface, resolvedMode === "dark" ? 0.085 : 0.095),
    "--text-primary": rgba(onSurface, 0.94),
    "--text-secondary": onSurfaceVariant,
    "--text-muted": rgba(onSurface, 0.48),
    "--border-subtle": rgba(outlineVariant, resolvedMode === "dark" ? 0.3 : 0.52),
    "--border-strong": rgba(onSurface, resolvedMode === "dark" ? 0.18 : 0.26),
    "--accent": primary,
    "--on-accent": onPrimary,
    "--accent-soft": rgba(primary, resolvedMode === "dark" ? 0.2 : 0.16),
    "--danger": error,
    "--danger-soft": rgba(error, resolvedMode === "dark" ? 0.15 : 0.12),
    "--app-glow": rgba(primary, resolvedMode === "dark" ? 0.12 : 0.18),
    "--shadow-card": resolvedMode === "dark"
      ? "0 16px 40px rgba(0, 0, 0, 0.24)"
      : "0 16px 38px rgba(31, 41, 55, 0.12)",
    "--shadow-soft": resolvedMode === "dark"
      ? "0 8px 24px rgba(0, 0, 0, 0.18)"
      : "0 8px 22px rgba(31, 41, 55, 0.10)",
    "--bg-color": surface,
    "--surface": surfaceContainerLow,
    "--surface-hover": rgba(onSurface, resolvedMode === "dark" ? 0.085 : 0.095),
    "--accent-transparent": rgba(primary, resolvedMode === "dark" ? 0.2 : 0.16),
    "--border": rgba(onSurface, resolvedMode === "dark" ? 0.1 : 0.16),
  };

  vars["--surface-panel"] = surfaceContainer;
  vars["--surface-panel-high"] = surfaceContainerHigh;

  return { mode: resolvedMode, vars };
};

export const applyAppTheme = (theme: AppThemeSnapshot) => {
  const root = document.documentElement;
  Object.entries(theme.vars).forEach(([key, value]) => {
    root.style.setProperty(key, value);
  });
  root.dataset.appThemeMode = theme.mode;
};

export const persistAppTheme = (theme: AppThemeSnapshot) => {
  localStorage.setItem(APP_THEME_STORAGE_KEY, JSON.stringify(theme));
};

export const restorePersistedAppTheme = () => {
  try {
    const raw = localStorage.getItem(APP_THEME_STORAGE_KEY);
    if (!raw) return;
    const theme = JSON.parse(raw) as AppThemeSnapshot;
    if (theme?.vars && theme?.mode) {
      applyAppTheme(theme);
    }
  } catch {
    localStorage.removeItem(APP_THEME_STORAGE_KEY);
  }
};
