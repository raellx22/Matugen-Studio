import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Image, Palette, LayoutTemplate, Monitor, Settings, Download, Copy, Edit2, Check, X, RotateCcw, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import Wheel from '@uiw/react-color-wheel';
import ShadeSlider from '@uiw/react-color-shade-slider';
import { hsvaToHex, hexToHsva, hexToRgba } from '@uiw/color-convert';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { openUrl } from '@tauri-apps/plugin-opener';
import { listen } from "@tauri-apps/api/event";
import Select from "./components/Select";
import { PageHeader, Section, SettingRow } from "./components/Primitives";
import AdvancedGeneration from "./pages/AdvancedGeneration";
import { defaultGeneration, defaultIntegrations, defaultStudioSettings, type GenerationSettings, type StudioSettings } from "./utils/studioSettings";
import Templates from "./pages/Templates";
import Source from "./pages/Source";
import WallpaperLibraryControls from "./pages/WallpaperLibraryControls";
import { initializeWallpaperLibrary } from "./utils/wallpaperLibrary";
import Presets from "./pages/Presets";
import KdeIntegration from "./pages/KdeIntegration";
import { applyAppTheme, buildAppTheme, persistAppTheme } from "./utils/appTheme";
import {
  DEFAULT_WALLPAPERS_PER_PAGE,
  WALLPAPERS_PER_PAGE_OPTIONS,
  validateWallpapersPerPage,
} from "./utils/wallpaperPagination";
import "./App.css";
import "./styles/v2.css";

const WALLPAPERS_PER_PAGE_STORAGE_KEY = "wallpapersPerPage";
const KDE_COLOR_OVERRIDES_STORAGE_KEY = "kdeColorOverrides";
const GTK_THEME_ENABLED_STORAGE_KEY = "gtkThemeEnabled";
const GTK_THEME_DARK_STORAGE_KEY = "gtkThemeDark";
const SIDEBAR_EXPANDED_STORAGE_KEY = "sidebarExpanded";
interface SchemeColorValue {
  hex?: string;
  color?: string;
  [key: string]: unknown;
}

interface SchemeColorGroup {
  dark?: SchemeColorValue;
  light?: SchemeColorValue;
  default?: SchemeColorValue;
}

interface SchemeData {
  colors: Record<string, SchemeColorGroup>;
  wallpaper_luminance?: number;
  source_color_luminance?: number;
  source_color_hex?: string;
  source_candidates?: { index: number; hex: string }[];
  seed_index?: number;
  generation_settings?: GenerationSettings;
  [key: string]: unknown;
}

const WALLHAVEN_API_KEY_STORAGE_KEY = "wallhavenApiKey";

type KdeColorControl = {
  key: string;
  label: string;
  path: string;
  fallback?: string;
  description: string;
};

const KDE_COLOR_CONTROLS: KdeColorControl[] = [
  { key: "windowBackground", label: "Window Background", path: "surface_container_low", fallback: "surface", description: "Main window and inactive panel background" },
  { key: "viewBackground", label: "View Background", path: "surface", description: "Lists, views, and content areas" },
  { key: "button", label: "Button", path: "surface_container", fallback: "surface", description: "Default button and menu surface" },
  { key: "selection", label: "Selection", path: "primary", description: "Selected items and active controls" },
  { key: "foreground", label: "Foreground", path: "on_surface", description: "Main readable text color" },
  { key: "link", label: "Link / Active", path: "link", fallback: "primary", description: "Links and active text accents" },
  { key: "negative", label: "Negative", path: "negative", fallback: "error", description: "Destructive and error actions" },
  { key: "neutral", label: "Neutral", path: "neutral", fallback: "tertiary", description: "Neutral status indicators" },
  { key: "visited", label: "Visited", path: "visited", fallback: "secondary", description: "Visited links and secondary active states" },
];

type SelectedColor = {
  name: string;
  path: string;
  hex: string;
  originalHex: string;
  scope: "matugen" | "template" | "kde";
  templateName?: string;
  templateDisplayName?: string;
  tokenKey?: string;
  description?: string | null;
  sourcePath?: string | null;
  overridden?: boolean;
};

interface TemplateColorControl {
  key: string;
  name: string;
  description: string | null;
  sourcePath: string | null;
  originalHex: string;
  currentHex: string;
  overridden: boolean;
}

interface TemplateColorGroup {
  templateName: string;
  displayName: string;
  targetApp: string;
  outputPath: string | null;
  controls: TemplateColorControl[];
}

interface KdeColorValue {
  key: string;
  hex: string;
}

type ToastTone = "success" | "error" | "info";

interface ToastMessage {
  id: number;
  message: string;
  tone: ToastTone;
}

function App() {
  const { t, i18n } = useTranslation();
  useEffect(() => { void initializeWallpaperLibrary().catch(error => console.error('Wallpaper library initialization failed:', error)); }, []);
  const [activeTab, setActiveTab] = useState("colors");
  const [wallpaperPath, setWallpaperPath] = useState<string | null>(null);
  const [schemeData, setSchemeData] = useState<SchemeData | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [schemeType, setSchemeType] = useState("Tinted Smart");
  const [mode, setMode] = useState<"dark" | "light" | "auto">("auto");

  const schemeGenerationRef = useRef(0);
  const [studioSettings, setStudioSettings] = useState<StudioSettings>(defaultStudioSettings);
  const studioSettingsRef = useRef(defaultStudioSettings);
  const [settingsReady, setSettingsReady] = useState(false);
  const settingsQueue = useRef<Promise<unknown>>(Promise.resolve());
  const persistSettings = (patch: Partial<StudioSettings>): Promise<StudioSettings> => {
    const next = settingsQueue.current.catch(() => undefined).then(async () => {
      const saved = await invoke<StudioSettings>("set_studio_settings", { settings: { ...studioSettingsRef.current, ...patch } });
      studioSettingsRef.current = saved;
      setStudioSettings(saved);
      return saved;
    });
    settingsQueue.current = next;
    return next;
  };
  useEffect(() => {
    let cancelled = false;
    invoke<StudioSettings>("get_studio_settings").then(async settings => {
      if (cancelled) return;
      // Migrate the existing frontend-only overrides and GTK preference once.
      if (settings.gtk_enabled === null) {
        const service = await invoke<{ gtk_enabled: boolean }>("get_kde_service_status");
        const migratedGtkEnabled = localStorage.getItem(GTK_THEME_ENABLED_STORAGE_KEY) === null ? service.gtk_enabled : gtkThemeEnabled;
        settings = await invoke<StudioSettings>("set_studio_settings", { settings: { ...settings, kde_overrides: kdeColorOverrides, gtk_enabled: migratedGtkEnabled } });
      }
      if (cancelled) return;
      studioSettingsRef.current = settings;
      setStudioSettings(settings);
      setMode(settings.mode);
      setSchemeType(settings.scheme_type);
      setKdeColorOverrides(settings.kde_overrides);
      setGtkThemeEnabled(settings.gtk_enabled ?? false);
      setSettingsReady(true);
    }).catch(e => notify("Failed to load color settings: " + e, "error"));
    const subscription = listen<{ wallpaper: string; context: SchemeData }>("studio-wallpaper-generated", event => {
      ++schemeGenerationRef.current;
      setWallpaperPath(event.payload.wallpaper);
      setSchemeData(event.payload.context);
      setIsLoading(false);
    });
    return () => { cancelled = true; void subscription.then(dispose => dispose()); };
  }, []);


  const effectiveMode = useMemo(() => {
    if (mode === "dark") return "dark";
    if (mode === "light") return "light";
    // Auto mode follows wallpaper-average luminance when available.
    const luma = schemeData?.wallpaper_luminance ?? schemeData?.source_color_luminance ?? 0;
    return luma > 0.5 ? "light" : "dark";
  }, [mode, schemeData]);
  const [selectedColor, setSelectedColor] = useState<SelectedColor | null>(null);
  const [showPicker, setShowPicker] = useState(false);
  const [pickerHsva, setPickerHsva] = useState({ h: 0, s: 0, v: 0, a: 1 });
  const [copied, setCopied] = useState(false);
  const [templateColorGroups, setTemplateColorGroups] = useState<TemplateColorGroup[]>([]);
  const [activeTemplateGroup, setActiveTemplateGroup] = useState<string>("");
  const [isLoadingTemplateColors, setIsLoadingTemplateColors] = useState(false);
  const [kdeColorOverrides, setKdeColorOverrides] = useState<Record<string, string>>(() => {
    try {
      return JSON.parse(localStorage.getItem(KDE_COLOR_OVERRIDES_STORAGE_KEY) ?? "{}");
    } catch {
      return {};
    }
  });
  const [kdeDiskColors, setKdeDiskColors] = useState<Record<string, string>>({});
  const [gtkThemeEnabled, setGtkThemeEnabled] = useState(() => localStorage.getItem(GTK_THEME_ENABLED_STORAGE_KEY) === "true");
  const [gtkThemeDark, setGtkThemeDark] = useState(() => localStorage.getItem(GTK_THEME_DARK_STORAGE_KEY) !== "false");
  const [sidebarExpanded, setSidebarExpanded] = useState(() => localStorage.getItem(SIDEBAR_EXPANDED_STORAGE_KEY) !== "false");
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const [wallpapersPerPage, setWallpapersPerPage] = useState(() => {
    const saved = localStorage.getItem(WALLPAPERS_PER_PAGE_STORAGE_KEY);
    const value = validateWallpapersPerPage(saved ?? DEFAULT_WALLPAPERS_PER_PAGE);
    localStorage.setItem(WALLPAPERS_PER_PAGE_STORAGE_KEY, String(value));
    return value;
  });

  const [wallhavenApiKeyInput, setWallhavenApiKeyInput] = useState(() => localStorage.getItem(WALLHAVEN_API_KEY_STORAGE_KEY) || "");
  const [wallhavenKeyStatus, setWallhavenKeyStatus] = useState<"idle" | "checking" | "valid" | "invalid">(
    () => (localStorage.getItem(WALLHAVEN_API_KEY_STORAGE_KEY) ? "valid" : "idle")
  );

  const notify = (message: string, tone: ToastTone = "info") => {
    const id = Date.now() + Math.random();
    setToasts(prev => [...prev.slice(-3), { id, message, tone }]);
    window.setTimeout(() => {
      setToasts(prev => prev.filter(toast => toast.id !== id));
    }, tone === "error" ? 5200 : 3200);
  };

  const handleWallpapersPerPageChange = (value: string) => {
    const parsed = validateWallpapersPerPage(value);
    setWallpapersPerPage(parsed);
    localStorage.setItem(WALLPAPERS_PER_PAGE_STORAGE_KEY, String(parsed));
  };

  const validateWallhavenKey = async () => {
    const key = wallhavenApiKeyInput.trim();
    if (!key) {
      localStorage.removeItem(WALLHAVEN_API_KEY_STORAGE_KEY);
      setWallhavenKeyStatus("idle");
      return;
    }
    setWallhavenKeyStatus("checking");
    try {
      await invoke("wallhaven_validate_key", { apiKey: key });
      localStorage.setItem(WALLHAVEN_API_KEY_STORAGE_KEY, key);
      setWallhavenKeyStatus("valid");
      notify(t('settings.wallhavenKeyValid'), "success");
    } catch (e) {
      setWallhavenKeyStatus("invalid");
      notify(t('settings.wallhavenKeyInvalid') + e, "error");
    }
  };

  const loadTemplateColorControls = async (contextOverride: any = schemeData) => {
    setIsLoadingTemplateColors(true);
    try {
      const groups = await invoke<TemplateColorGroup[]>("list_template_color_controls", { context: contextOverride ?? null });
      setTemplateColorGroups(groups);
      setActiveTemplateGroup(current => {
        if (current && groups.some(group => group.templateName === current)) {
          return current;
        }
        return groups[0]?.templateName ?? "";
      });
    } catch (err) {
      console.error("Failed to load template color controls", err);
      setTemplateColorGroups([]);
      setActiveTemplateGroup("");
    } finally {
      setIsLoadingTemplateColors(false);
    }
  };

  useEffect(() => {
    if (activeTab === "colors") {
      void loadTemplateColorControls();
      void loadKdeDiskColors();
    }
  }, [activeTab, schemeData]);

  useEffect(() => {
    if (!schemeData) return;
    const theme = buildAppTheme(schemeData, mode);
    applyAppTheme(theme);
    persistAppTheme(theme);
  }, [schemeData, mode]);

  const loadKdeDiskColors = async () => {
    try {
      const values = await invoke<KdeColorValue[]>("get_kde_color_values");
      setKdeDiskColors(Object.fromEntries(values.map(value => [value.key, value.hex])));
    } catch (err) {
      console.error("Failed to load KDE color values", err);
      setKdeDiskColors({});
    }
  };

  const selectedTemplateGroup = useMemo(
    () => templateColorGroups.find(group => group.templateName === activeTemplateGroup) ?? null,
    [templateColorGroups, activeTemplateGroup],
  );

  const templateOverrideCount = useMemo(
    () => templateColorGroups.reduce((total, group) => total + group.controls.filter(control => control.overridden).length, 0),
    [templateColorGroups],
  );

  const normalizeHex = (hex?: string | null) => {
    if (!hex) return null;
    const value = hex.trim();
    if (value.length === 9 && value.startsWith('#')) {
      return `#${value.slice(1, 7)}`.toUpperCase();
    }
    if (value.length === 7 && value.startsWith('#')) {
      return value.toUpperCase();
    }
    return null;
  };

  const readSchemeColor = (data: any, path: string, fallback?: string) => {
    const read = (key: string) => {
      const color = data?.colors?.[key];
      if (!color) return null;
      for (const variant of ["default", "dark", "light"]) {
        const value = normalizeHex(color[variant]?.hex || color[variant]?.color);
        if (value) return value;
      }
      return null;
    };

    return read(path) || (fallback ? read(fallback) : null) || "#333333";
  };

  const buildThemeContext = (baseContext: any = schemeData, modeOverride: "dark" | "light" | "auto" = mode) => {
    if (!baseContext) return baseContext;
    const context = JSON.parse(JSON.stringify(baseContext));

    // Compute the correct mode immediately for this context data to avoid React state delays
    const luma = baseContext.wallpaper_luminance ?? baseContext.source_color_luminance ?? 0;
    const targetMode = modeOverride === "auto" ? (luma > 0.5 ? "light" : "dark") : modeOverride;

    // Ensure the 'default' variant for each color group correctly reflects our computed light/dark mode
    if (context.colors) {
      for (const key in context.colors) {
        if (context.colors[key][targetMode]) {
          context.colors[key].default = { ...context.colors[key][targetMode] };
        }
      }
    }

    return context;
  };

  // Regenerates the KDE .colors file from clean theme data, then layers the
  // user's per-control overrides on top via precise single-property ini
  // patches (see kde_color_mappings in src-tauri/src/commands/kde.rs). This
  // keeps each control (e.g. "Window Background") isolated to the single
  // KDE property it is labeled for, instead of bleeding into every other
  const applyKdeSchemeWithOverrides = async (
    baseContext: Record<string, unknown> | null | undefined,
    overrides: Record<string, string> = kdeColorOverrides,
  ) => {
    if (!baseContext) return;
    await invoke("apply_kde_colorscheme", { context: baseContext });
    const warnings = await invoke<string[]>("apply_kde_integrations", { context: baseContext });
    warnings.forEach(warning => notify(warning, "info"));
    const overrideValues = KDE_COLOR_CONTROLS
      .map(control => ({ key: control.key, hex: normalizeHex(overrides[control.key]) }))
      .filter((entry): entry is { key: string; hex: string } => Boolean(entry.hex));
    if (overrideValues.length > 0) {
      await invoke("apply_kde_color_values", { values: overrideValues });
    }
  };

  const saveKdeColorOverrides = (next: Record<string, string>) => {
    setKdeColorOverrides(next);
    localStorage.setItem(KDE_COLOR_OVERRIDES_STORAGE_KEY, JSON.stringify(next));
    void persistSettings({ kde_overrides: next }).catch(e => notify(String(e), "error"));
  };

  const getKdeControlColor = (control: KdeColorControl, overrides: Record<string, string> = kdeColorOverrides, data: any = schemeData) => {
    return normalizeHex(overrides[control.key]) || normalizeHex(kdeDiskColors[control.key]) || readSchemeColor(data, control.path, control.fallback);
  };

  const persistGtkThemeEnabled = (enabled: boolean) => {
    setGtkThemeEnabled(enabled);
    localStorage.setItem(GTK_THEME_ENABLED_STORAGE_KEY, String(enabled));
    void persistSettings({ gtk_enabled: enabled }).catch(e => notify(String(e), "error"));
  };

  const persistGtkThemeDark = (dark: boolean) => {
    setGtkThemeDark(dark);
    localStorage.setItem(GTK_THEME_DARK_STORAGE_KEY, String(dark));
  };

  const toggleSidebarExpanded = () => {
    setSidebarExpanded(prev => {
      const next = !prev;
      localStorage.setItem(SIDEBAR_EXPANDED_STORAGE_KEY, String(next));
      return next;
    });
  };

  const getEffectiveGtkDark = (data: any, modeOverride: "dark" | "light" | "auto" = mode) => {
    const luma = data?.wallpaper_luminance ?? data?.source_color_luminance ?? 0;
    return modeOverride === "auto" ? luma <= 0.5 : modeOverride === "dark";
  };

  const applyGeneratedThemeContext = async (data: any, modeOverride: "dark" | "light" | "auto" = mode) => {
    const targetDark = getEffectiveGtkDark(data, modeOverride);
    const themeContext = buildThemeContext(data, modeOverride);
    const tasks: Promise<unknown>[] = [
      invoke("apply_theme", { context: themeContext }),
      applyKdeSchemeWithOverrides(themeContext, kdeColorOverrides),
    ];

    if (gtkThemeEnabled) {
      persistGtkThemeDark(targetDark);
      tasks.push(invoke("apply_gtk_theme", { context: themeContext, dark: targetDark }));
    }

    const results = await Promise.allSettled(tasks);
    const failed = results.find(result => result.status === "rejected");
    await Promise.allSettled([
      loadTemplateColorControls(themeContext),
      loadKdeDiskColors(),
    ]);
    if (failed && failed.status === "rejected") {
      throw failed.reason;
    }
    return targetDark;
  };

  const handleModeChange = async (nextMode: "dark" | "light" | "auto") => {
    try { await persistSettings({ mode: nextMode }); } catch (e) { notify(String(e), "error"); return; }
    setMode(nextMode);
    if (!schemeData) return;

    try {
      setIsLoading(true);
      await applyGeneratedThemeContext(schemeData, nextMode);
      notify(t('messages.modeReapplied'), "success");
    } catch (err: any) {
      setErrorMsg(err.toString());
      notify(t('messages.failedToApplyTheme') + err, "error");
    } finally {
      setIsLoading(false);
    }
  };

  const selectImage = async () => {
    let generation: number | null = null;
    try {
      const file = await open({
        multiple: false,
        filters: [{
          name: 'Image',
          extensions: ['png', 'jpeg', 'jpg', 'webp']
        }]
      });

      if (file) {
        // file could be a string or array, we set multiple: false so it's string
        const path = file as string;
        generation = ++schemeGenerationRef.current;
        setWallpaperPath(path);
        setIsLoading(true);

        const data = await invoke<SchemeData>("generate_scheme_from_image", { imagePath: path, schemeType, settings: studioSettingsRef.current.generation });
        if (schemeGenerationRef.current !== generation) return;
        setSchemeData(data);
        setErrorMsg(null);
      }
    } catch (err: unknown) {
      console.error("Failed to select image or generate scheme", err);
      if (generation === null || schemeGenerationRef.current === generation) {
        setErrorMsg(String(err));
      }
    } finally {
      if (generation === null || schemeGenerationRef.current === generation) {
        setIsLoading(false);
      }
    }
  };

  // Inject .color to all color groups so Matugen parses them correctly
  const fixMatugenColors = (data: SchemeData): SchemeData => {
    const cloned = structuredClone(data);
    for (const key in cloned.colors) {
      const group = cloned.colors[key];
      if (group.dark?.hex) group.dark.color = group.dark.hex;
      if (group.light?.hex) group.light.color = group.light.hex;
      if (group.default?.hex) group.default.color = group.default.hex;
    }
    return cloned;
  };

  const handleSchemeTypeChange = async (newType: string) => {
    const generation = ++schemeGenerationRef.current;
    try { await persistSettings({ scheme_type: newType }); } catch (e) { notify(String(e), "error"); return; }
    setSchemeType(newType);
    if (wallpaperPath) {
      try {
        setIsLoading(true);
        const rawData = await invoke<SchemeData>("generate_scheme_from_image", { imagePath: wallpaperPath, schemeType: newType, settings: studioSettingsRef.current.generation });
        if (schemeGenerationRef.current !== generation) return;
        const data = fixMatugenColors(rawData);
        setSchemeData(data);
        setErrorMsg(null);
        await applyGeneratedThemeContext(data);
        if (schemeGenerationRef.current !== generation) return;
        notify(t('messages.schemeRegenerated', { scheme: newType }), "success");
      } catch (err: unknown) {
        if (schemeGenerationRef.current !== generation) return;
        setErrorMsg(String(err));
        notify(t('messages.failedToGenerate') + err, "error");
      } finally {
        if (schemeGenerationRef.current === generation) setIsLoading(false);
      }
    }
  };

  const handleGenerationChange = async (generation: GenerationSettings) => {
    if (!settingsReady || isLoading) return;
    const request = ++schemeGenerationRef.current;
    setIsLoading(true);
    try {
      await persistSettings({ generation });
      if (wallpaperPath) {
        const data = fixMatugenColors(await invoke<SchemeData>("generate_scheme_from_image", { imagePath: wallpaperPath, schemeType, settings: generation }));
        if (request !== schemeGenerationRef.current) return;
        setSchemeData(data);
        await applyGeneratedThemeContext(data);
      }
      setErrorMsg(null);
    } catch (e) { setErrorMsg(String(e)); notify(String(e), "error"); }
    finally { if (request === schemeGenerationRef.current) setIsLoading(false); }
  };

  const handleApplyAndGenerate = async (path: string) => {
    const generation = ++schemeGenerationRef.current;
    setWallpaperPath(path);
    try {
      setIsLoading(true);
      await invoke("mark_kde_wallpaper_handled", { wallpaperPath: path }).catch(e =>
        console.warn("Failed to sync KDE watcher state:", e)
      );
      const rawData = await invoke<SchemeData>("generate_scheme_from_image", { imagePath: path, schemeType, settings: studioSettingsRef.current.generation });
      if (schemeGenerationRef.current !== generation) return;
      const data = fixMatugenColors(rawData);
      setSchemeData(data);
      setErrorMsg(null);
      await applyGeneratedThemeContext(data);
      if (schemeGenerationRef.current !== generation) return;
      notify(t('messages.themeGenerated'), "success");
    } catch (err: unknown) {
      if (schemeGenerationRef.current !== generation) return;
      setErrorMsg(String(err));
      notify(t('messages.failedToGenerate') + err, "error");
    } finally {
      if (schemeGenerationRef.current === generation) setIsLoading(false);
    }
  };

  const handleSelectForColors = async (path: string) => {
    const generation = ++schemeGenerationRef.current;
    setWallpaperPath(path);
    setActiveTab('colors');
    try {
      setIsLoading(true);
      const rawData = await invoke<SchemeData>("generate_scheme_from_image", { imagePath: path, schemeType, settings: studioSettingsRef.current.generation });
      if (schemeGenerationRef.current !== generation) return;
      const data = fixMatugenColors(rawData);
      setSchemeData(data);
      setErrorMsg(null);
    } catch (err: unknown) {
      if (schemeGenerationRef.current !== generation) return;
      setErrorMsg(String(err));
      notify(t('messages.failedToGenerateColors') + err, "error");
    } finally {
      if (schemeGenerationRef.current === generation) setIsLoading(false);
    }
  };

  // Helper to update all fields Matugen Tera templates might use
  const updateMatugenColor = (colorObj: any, hex: string) => {
    if (!colorObj) return;
    const stripped = hex.replace('#', '');
    const r = parseInt(stripped.substring(0, 2), 16);
    const g = parseInt(stripped.substring(2, 4), 16);
    const b = parseInt(stripped.substring(4, 6), 16);
    
    // IMPORTANT: Matugen's resolve_path checks if a map has a "color" key.
    // If it does, it treats it as a custom color and uses colorsys to format it correctly!
    // Without this, Matugen core fails to parse the mutated JSON map and prints an IndexMap debug string.
    colorObj.color = hex; 
    
    colorObj.hex = hex;
    colorObj.hex_stripped = stripped;
    colorObj.hex_alpha = hex + 'ff';
    colorObj.hex_alpha_stripped = stripped + 'ff';
    colorObj.rgb = `rgb(${r}, ${g}, ${b})`;
    colorObj.rgba = `rgba(${r}, ${g}, ${b}, 255)`;
    colorObj.red = r.toString();
    colorObj.green = g.toString();
    colorObj.blue = b.toString();
  };

  const handleCustomColorChange = (hex: string) => {
    if (selectedColor) {
      if (selectedColor.scope === "matugen") {
        setSchemeData((prev: any) => {
          if (!prev) return prev;
          const newData = JSON.parse(JSON.stringify(prev));
          const group = newData.colors[selectedColor.path];
          if (group) {
            updateMatugenColor(group.dark, hex);
            updateMatugenColor(group.light, hex);
            updateMatugenColor(group.default, hex);
          }
          return newData;
        });
      }
      setSelectedColor(prev => prev ? { ...prev, hex } : null);
    }
  };

  const applyTemplateOverride = async (color: SelectedColor, hex: string) => {
    if (color.scope !== "template" || !color.templateName || !color.tokenKey) {
      return;
    }

    await invoke("set_template_color_override", {
      templateName: color.templateName,
      tokenKey: color.tokenKey,
      hex,
      originalHex: color.originalHex,
    });
    if (schemeData) {
      await invoke("apply_theme", { context: buildThemeContext(schemeData) });
    } else {
      await invoke("apply_template_overrides_to_outputs");
    }
    await loadTemplateColorControls();
  };

  const resetTemplateOverride = async (templateName: string, tokenKey?: string) => {
    await invoke("reset_template_color_override", {
      templateName,
      tokenKey: tokenKey ?? null,
    });
    if (schemeData) {
      await invoke("apply_theme", { context: buildThemeContext(schemeData) });
    } else {
      await invoke("apply_template_overrides_to_outputs");
    }
    await loadTemplateColorControls();
  };

  const applyKdeColorOverride = async (color: SelectedColor, hex: string) => {
    if (!color.tokenKey) {
      return;
    }

    const normalized = normalizeHex(hex);
    if (!normalized) return;
    const next = { ...kdeColorOverrides, [color.tokenKey]: normalized };
    saveKdeColorOverrides(next);
    if (schemeData) {
      await applyKdeSchemeWithOverrides(buildThemeContext(schemeData), next);
    } else {
      await invoke("apply_kde_color_values", { values: Object.entries(next).map(([key, hex]) => ({ key, hex })) });
    }
    await loadKdeDiskColors();
  };

  const resetKdeColorOverride = async (controlKey?: string) => {
    const next = { ...kdeColorOverrides };
    if (controlKey) {
      delete next[controlKey];
    } else {
      KDE_COLOR_CONTROLS.forEach(control => delete next[control.key]);
    }
    saveKdeColorOverrides(next);
    if (schemeData) {
      await applyKdeSchemeWithOverrides(buildThemeContext(schemeData), next);
    } else {
      await invoke("apply_kde_color_values", { values: Object.entries(next).map(([key, hex]) => ({ key, hex })) });
    }
    await loadKdeDiskColors();
  };

  const handleApplyPreset = async (preset: any) => {
    const wallpaper = preset.wallpaper?.original_path ?? null;
    const scheme_data = preset.scheme.scheme_data;
    const scheme_type = preset.scheme.scheme_type;

    ++schemeGenerationRef.current;
    try {
      await persistSettings({
        generation: { ...defaultGeneration, ...Object.fromEntries(Object.keys(defaultGeneration).map(key => [key, preset.scheme[key] ?? defaultGeneration[key as keyof GenerationSettings]])) } as GenerationSettings,
        integrations: preset.desktop?.integrations ?? defaultIntegrations,
        mode: preset.scheme.mode ?? "dark",
        scheme_type,
      });
    } catch (e) { notify(String(e), "error"); return; }
    setWallpaperPath(wallpaper);
    setSchemeType(scheme_type);
    setSchemeData(scheme_data);
    setMode((preset.scheme.mode ?? "dark") as "dark" | "light" | "auto");

    const desktop = preset.desktop ?? { apply_wallpaper: true, apply_kde_colorscheme: true, apply_templates: true };
    const presetMode = (preset.scheme.mode ?? "dark") as "dark" | "light" | "auto";
    const themedPresetContext = buildThemeContext(scheme_data, presetMode);

    try {
      if (wallpaper && desktop.apply_wallpaper) {
        const stableWallpaper = await invoke<string>("prepare_wallpaper_for_kde", { imagePath: wallpaper });
        await invoke("apply_wallpaper", { imagePath: stableWallpaper, screenIndex: -1 });
        setWallpaperPath(stableWallpaper);
      }
      if (desktop.apply_templates) {
        await invoke("apply_theme", { context: themedPresetContext });
      }
      if (desktop.apply_kde_colorscheme) {
        await applyKdeSchemeWithOverrides(themedPresetContext, kdeColorOverrides);
      }
      if (gtkThemeEnabled) {
        await invoke("apply_gtk_theme", { context: themedPresetContext, dark: getEffectiveGtkDark(scheme_data, presetMode) });
      }
      notify(t('messages.presetApplied'), "success");
    } catch (e) {
      notify(t('messages.failedToApplyPreset') + e, "error");
    }
  };

  const copyColor = async (hex: string) => {
    try {
      await writeText(hex);
      setCopied(true);
      setTimeout(() => {
        setCopied(false);
        setSelectedColor(null);
      }, 1000);
    } catch (err: any) {
      console.error(err);
      setErrorMsg("Failed to copy: " + err.toString());
    }
  };

  const renderColorSwatch = (name: string, path: string, isLarge = false) => {
    let hex = "#333333";
    if (schemeData && schemeData.colors && schemeData.colors[path]) {
      hex = schemeData.colors[path][effectiveMode]?.hex || schemeData.colors[path][effectiveMode]?.color || "#333333";
    }
    // Calculate contrast text color
    const textColor = isLightColor(hex) ? '#000000' : '#ffffff';

    return (
      <button type="button"
        className={`color-swatch ${isLarge ? 'large' : ''}`} 
        style={{ backgroundColor: hex, color: textColor }}
        title={`${name}: ${hex.toUpperCase()}`}
        aria-label={`${name}: ${hex.toUpperCase()}`}
        onClick={() => { 
          setSelectedColor({name, path, hex, originalHex: hex, scope: "matugen"});
          setPickerHsva(hexToHsva(hex));
          setShowPicker(false); 
        }}
      >
        <span style={{opacity: 0.7}}>{name}</span>
        <span style={{fontWeight: 'bold'}}>{hex.toUpperCase()}</span>
      </button>
    );
  };

  const renderTemplateColorToken = (group: TemplateColorGroup, control: TemplateColorControl) => {
    const hex = control.currentHex || control.originalHex;
    const textColor = isLightColor(hex) ? '#000000' : '#ffffff';

    return (
      <button
        key={control.key}
        type="button"
        className={`template-token-card ${control.overridden ? 'is-overridden' : ''}`}
        onClick={() => {
          setSelectedColor({
            name: control.name,
            path: control.key,
            hex,
            originalHex: control.originalHex,
            scope: "template",
            templateName: group.templateName,
            templateDisplayName: group.displayName,
            tokenKey: control.key,
            description: control.description,
            sourcePath: control.sourcePath,
            overridden: control.overridden,
          });
          setPickerHsva(hexToHsva(hex));
          setShowPicker(false);
        }}
      >
        <span className="template-token-swatch" style={{ backgroundColor: hex, color: textColor }}>
          {hex.toUpperCase()}
        </span>
        <span className="template-token-body">
          <span className="template-token-name">{control.name}</span>
          {control.description && <span className="template-token-description">{control.description}</span>}
          <span className="template-token-source">{control.sourcePath ?? "rendered color"}</span>
        </span>
        {control.overridden && (
          <span
            className="template-token-reset"
            role="button"
            tabIndex={0}
            title="Reset override"
            onClick={(event) => {
              event.stopPropagation();
              void resetTemplateOverride(group.templateName, control.key)
                .then(() => notify("Override reset.", "success"))
                .catch(e => notify("Failed to reset override: " + e, "error"));
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.stopPropagation();
                void resetTemplateOverride(group.templateName, control.key)
                  .then(() => notify("Override reset.", "success"))
                  .catch(e => notify("Failed to reset override: " + e, "error"));
              }
            }}
          >
            <RotateCcw size={14} />
          </span>
        )}
      </button>
    );
  };

  const renderTemplateColorEditor = () => {
    return (
      <div className="card template-colors-panel">
        <div className="template-colors-header">
          <div>
            <h3>{t('colors.templateOverridesTitle')}</h3>
            <p>{t('colors.templateOverridesDesc')}</p>
          </div>
          <div className="template-colors-summary">
            <span>{t('colors.templatesCount', { count: templateColorGroups.length })}</span>
            <span>{t('colors.overridesCount', { count: templateOverrideCount })}</span>
          </div>
        </div>

        {isLoadingTemplateColors ? (
          <div className="template-colors-empty">
            <Palette size={28} opacity={0.35} />
            <span>{t('colors.scanningTemplates')}</span>
          </div>
        ) : templateColorGroups.length === 0 ? (
          <div className="template-colors-empty">
            <LayoutTemplate size={28} opacity={0.35} />
            <span>{t('colors.installTemplatesToEdit')}</span>
          </div>
        ) : (
          <>
            <div className="template-colors-toolbar">
              <Select label="Template" value={activeTemplateGroup} onValueChange={setActiveTemplateGroup}
                options={templateColorGroups.map(group => ({ value: group.templateName, label: `${group.displayName} · ${group.controls.length}` }))} />
              {selectedTemplateGroup && (
                <button
                  type="button"
                  className="btn btn-secondary btn-compact"
                  disabled={!selectedTemplateGroup.controls.some(control => control.overridden)}
                  onClick={() => resetTemplateOverride(selectedTemplateGroup.templateName)
                    .then(() => notify("Template overrides reset.", "success"))
                    .catch(e => notify("Failed to reset overrides: " + e, "error"))}
                >
                  <RotateCcw size={14} />
                  {t('colors.resetTemplate')}
                </button>
              )}
            </div>

            {selectedTemplateGroup && (
              <>
                <div className="template-colors-meta">
                  <span>{selectedTemplateGroup.targetApp}</span>
                  {selectedTemplateGroup.outputPath && <span>{selectedTemplateGroup.outputPath}</span>}
                </div>
                <div className="template-token-grid">
                  {selectedTemplateGroup.controls.map(control => renderTemplateColorToken(selectedTemplateGroup, control))}
                </div>
              </>
            )}
          </>
        )}
      </div>
    );
  };

  const renderKdeColorEditor = () => {
    const overrideCount = KDE_COLOR_CONTROLS.filter(control => kdeColorOverrides[control.key]).length;
    const hasKdeColors = Boolean(schemeData) || Object.keys(kdeDiskColors).length > 0;

    return (
      <div className="card kde-colors-panel">
        <div className="template-colors-header">
          <div>
            <h3>KDE Plasma Colors</h3>
            <p>Adjust the colors used by the generated KDE color scheme, without changing template outputs.</p>
          </div>
          <div className="template-colors-summary">
            <span>{KDE_COLOR_CONTROLS.length} colors</span>
            <span>{overrideCount} overrides</span>
          </div>
        </div>

        <div className="template-colors-toolbar">
          <div className="template-colors-meta">
            <span>MatugenStudio.colors</span>
            <span>Qt/KDE apps</span>
          </div>
          <button
            type="button"
            className="btn btn-secondary btn-compact"
            disabled={overrideCount === 0}
            onClick={() => resetKdeColorOverride()
              .then(() => notify("KDE color overrides reset.", "success"))
              .catch(e => notify("Failed to reset KDE colors: " + e, "error"))}
          >
            <RotateCcw size={14} />
            Reset KDE
          </button>
        </div>

        {!hasKdeColors ? (
          <div className="template-colors-empty">
            <Monitor size={28} opacity={0.35} />
            <span>Generate or apply a KDE scheme to edit KDE colors here.</span>
          </div>
        ) : (
          <div className="template-token-grid kde-token-grid">
            {KDE_COLOR_CONTROLS.map(control => {
              const originalHex = normalizeHex(kdeDiskColors[control.key]) || readSchemeColor(schemeData, control.path, control.fallback);
              const hex = getKdeControlColor(control);
              const overridden = Boolean(kdeColorOverrides[control.key]);
              const textColor = isLightColor(hex) ? '#000000' : '#ffffff';

              return (
                <button
                  key={control.key}
                  type="button"
                  className={`template-token-card ${overridden ? 'is-overridden' : ''}`}
                  onClick={() => {
                    setSelectedColor({
                      name: control.label,
                      path: control.path,
                      hex,
                      originalHex,
                      scope: "kde",
                      tokenKey: control.key,
                      description: control.description,
                      sourcePath: control.path,
                      overridden,
                    });
                    setPickerHsva(hexToHsva(hex));
                    setShowPicker(false);
                  }}
                >
                  <span className="template-token-swatch" style={{ backgroundColor: hex, color: textColor }}>
                    {hex.toUpperCase()}
                  </span>
                  <span className="template-token-body">
                    <span className="template-token-name">{control.label}</span>
                    <span className="template-token-description">{control.description}</span>
                    <span className="template-token-source">{control.path}</span>
                  </span>
                  {overridden && (
                    <span
                      className="template-token-reset"
                      role="button"
                      tabIndex={0}
                      title="Reset KDE color"
                      onClick={(event) => {
                        event.stopPropagation();
                        void resetKdeColorOverride(control.key)
                          .then(() => notify("KDE color reset.", "success"))
                          .catch(e => notify("Failed to reset KDE color: " + e, "error"));
                      }}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.stopPropagation();
                          void resetKdeColorOverride(control.key)
                            .then(() => notify("KDE color reset.", "success"))
                            .catch(e => notify("Failed to reset KDE color: " + e, "error"));
                        }
                      }}
                    >
                      <RotateCcw size={14} />
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        )}
      </div>
    );
  };

  // Helper to determine text color
  const isLightColor = (hex: string) => {
    const c = hex.substring(1);
    const rgb = parseInt(c, 16);
    const r = (rgb >> 16) & 0xff;
    const g = (rgb >>  8) & 0xff;
    const b = (rgb >>  0) & 0xff;
    const luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    return luma > 128;
  };

  const currentHex = selectedColor ? hsvaToHex(pickerHsva) : "";
  const currentRgb = currentHex ? hexToRgba(currentHex) : {r:0,g:0,b:0};
  
  return (
    <div className="app-container">
      <div className="toast-stack" aria-live="polite" aria-atomic="true">
        {toasts.map(toast => (
          <div key={toast.id} className={`toast toast-${toast.tone}`}>
            {toast.message}
          </div>
        ))}
      </div>

      {selectedColor && (
        <div className="modal-overlay" onClick={() => { setSelectedColor(null); setShowPicker(false); }}>
          <div className="modal-content" role="dialog" aria-modal="true" aria-label={selectedColor.name} onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <div>
                <h3>{selectedColor.name.replace('-', ' ')}</h3>
                {selectedColor.scope === "template" && (
                  <p style={{ margin: '4px 0 0', color: 'var(--text-secondary)', fontSize: 12 }}>
                    {selectedColor.templateDisplayName}
                  </p>
                )}
                {selectedColor.scope === "kde" && (
                  <p style={{ margin: '4px 0 0', color: 'var(--text-secondary)', fontSize: 12 }}>
                    KDE Plasma Colors
                  </p>
                )}
              </div>
              <button type="button" className="btn btn-ghost icon-btn" aria-label="Close" onClick={() => { setSelectedColor(null); setShowPicker(false); }}><X size={20} /></button>
            </div>

            <div className="color-preview-box">
              <div className="color-preview-half" style={{ backgroundColor: selectedColor.originalHex, color: isLightColor(selectedColor.originalHex) ? '#000' : '#fff' }}>
                {selectedColor.originalHex.toUpperCase()}
              </div>
              <div className="color-preview-half" style={{ backgroundColor: currentHex, color: isLightColor(currentHex) ? '#000' : '#fff' }}>
                {currentHex.toUpperCase()}
              </div>
            </div>

            <div className="color-info-grid">
              <div className="color-info-col">
                <span className="color-info-label">HEX</span>
                <span className="color-info-value">{currentHex.toUpperCase()}</span>
              </div>
              <div className="color-info-col">
                <span className="color-info-label">RGB</span>
                <span className="color-info-value">{currentRgb.r}, {currentRgb.g}, {currentRgb.b}</span>
              </div>
              <div className="color-info-col">
                <span className="color-info-label">HSV</span>
                <span className="color-info-value">{Math.round(pickerHsva.h)}°, {Math.round(pickerHsva.s)}%, {Math.round(pickerHsva.v)}%</span>
              </div>
            </div>

            {(selectedColor.scope === "template" || selectedColor.scope === "kde") && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6, padding: 12, borderRadius: 12, background: 'var(--surface-elevated)', border: '1px solid var(--border)' }}>
                {selectedColor.description && (
                  <span style={{ color: 'var(--text-primary)', fontSize: 13 }}>{selectedColor.description}</span>
                )}
                {selectedColor.sourcePath && (
                  <span style={{ color: 'var(--text-secondary)', fontSize: 12, fontFamily: 'monospace' }}>{selectedColor.sourcePath}</span>
                )}
              </div>
            )}

            {!showPicker ? (
              <div className="modal-actions">
                <button className="btn btn-secondary" onClick={() => { setPickerHsva(hexToHsva(selectedColor.hex)); setShowPicker(true); }}>
                  <Edit2 size={16} /> Edit Color
                </button>
                <button className="btn btn-primary" onClick={() => copyColor(currentHex)}>
                  {copied ? <Check size={16} /> : <Copy size={16} />} 
                  {copied ? "Copied!" : "Copy Code"}
                </button>
              </div>
            ) : (
              <div className="wheel-container">
                <Wheel 
                  color={pickerHsva} 
                  onChange={(color) => {
                    if (color && color.hsva) {
                      setPickerHsva(color.hsva);
                      handleCustomColorChange(color.hex);
                    }
                  }} 
                />
                <ShadeSlider 
                  hsva={pickerHsva} 
                  style={{ width: '100%', marginTop: 8 }}
                  onChange={(newShade) => {
                    if (newShade && typeof newShade.v === 'number') {
                      const newHsva = { ...pickerHsva, v: newShade.v };
                      setPickerHsva(newHsva);
                      handleCustomColorChange(hsvaToHex(newHsva));
                    }
                  }} 
                />
                <button 
                  className="btn btn-primary" 
                  style={{width: '100%'}} 
                  onClick={async () => { 
                    try {
                      if (selectedColor.scope === "template") {
                        await applyTemplateOverride(selectedColor, currentHex);
                      } else if (selectedColor.scope === "kde") {
                        await applyKdeColorOverride(selectedColor, currentHex);
                      } else {
                        if (!schemeData) {
                          notify(t('messages.generateFirst'), "error");
                          return;
                        }
                        await invoke("apply_theme", { context: buildThemeContext(schemeData) });
                      }
                      notify(selectedColor.scope === "template" || selectedColor.scope === "kde" ? t('colors.saveOverride') : t('colors.applyChanges'), "success");
                      setSelectedColor(null);
                      setShowPicker(false);
                    } catch (e) {
                      console.error(e);
                      notify("Failed to apply color change: " + e, "error");
                    }
                  }}
                >
                  <Check size={16} /> {selectedColor.scope === "template" || selectedColor.scope === "kde" ? t('colors.saveOverride') : t('colors.applyChanges')}
                </button>
              </div>
            )}
          </div>
        </div>
      )}

      <aside className={`sidebar ${sidebarExpanded ? 'expanded' : ''}`}>
        <div className="sidebar-header">
          <div className="sidebar-logo">
            <img src="/matugen-studio.png" alt="" width={28} height={28} />
          </div>
          <span className="sidebar-brand">Matugen Studio</span>
        </div>
        <button
          type="button"
          className="sidebar-toggle"
          onClick={toggleSidebarExpanded}
          title={sidebarExpanded ? t('sidebar.collapse') : t('sidebar.expand')}
        >
          {sidebarExpanded ? <PanelLeftClose size={18} /> : <PanelLeftOpen size={18} />}
        </button>
        <nav className="sidebar-nav">
          <button type="button" className={`sidebar-item ${activeTab === 'source' ? 'active' : ''}`} onClick={() => setActiveTab('source')} title={t('sidebar.source')} aria-current={activeTab === 'source' ? 'page' : undefined}>
            <Image size={22} />
            <span>{t('sidebar.source')}</span>
          </button>
          <button type="button" className={`sidebar-item ${activeTab === 'presets' ? 'active' : ''}`} onClick={() => setActiveTab('presets')} title={t('sidebar.presets')} aria-current={activeTab === 'presets' ? 'page' : undefined}>
            <Download size={22} />
            <span>{t('sidebar.presets')}</span>
          </button>
          <button type="button" className={`sidebar-item ${activeTab === 'colors' ? 'active' : ''}`} onClick={() => setActiveTab('colors')} title={t('sidebar.colors')} aria-current={activeTab === 'colors' ? 'page' : undefined}>
            <Palette size={22} />
            <span>{t('sidebar.colors')}</span>
          </button>
          <button type="button" className={`sidebar-item ${activeTab === 'templates' ? 'active' : ''}`} onClick={() => setActiveTab('templates')} title={t('sidebar.templates')} aria-current={activeTab === 'templates' ? 'page' : undefined}>
            <LayoutTemplate size={22} />
            <span>{t('sidebar.templates')}</span>
          </button>
          <button type="button" className={`sidebar-item ${activeTab === 'desktop' ? 'active' : ''}`} onClick={() => setActiveTab('desktop')} title={t('sidebar.desktop')} aria-current={activeTab === 'desktop' ? 'page' : undefined}>
            <Monitor size={22} />
            <span>{t('sidebar.desktop')}</span>
          </button>
        </nav>
        <div style={{ flex: 1 }}></div>
        <nav className="sidebar-nav">
          <button type="button"
            className="sidebar-item sidebar-apply"
            title={t('sidebar.apply')}
            onClick={async () => {
              if (!schemeData) {
                notify(t('messages.generateFirst'), "error");
                return;
              }
              try {
                const tasks: Promise<any>[] = [
                  invoke("apply_theme", { context: buildThemeContext(schemeData) }),
                  applyKdeSchemeWithOverrides(buildThemeContext(schemeData), kdeColorOverrides),
                ];
                if (gtkThemeEnabled) {
                  tasks.push(invoke("apply_gtk_theme", { context: buildThemeContext(schemeData), dark: getEffectiveGtkDark(schemeData) }));
                }
                const results = await Promise.allSettled(tasks);
                const failed = results.find(result => result.status === "rejected");
                if (failed && failed.status === "rejected") {
                  throw failed.reason;
                }
                notify(t('messages.themeAppliedGlobally'), "success");
              } catch (e) {
                notify(t('messages.failedToApplyTheme') + e, "error");
              }
            }}
          >
            <Download size={22} />
            <span>{t('sidebar.apply')}</span>
          </button>
          <button type="button" className={`sidebar-item ${activeTab === 'settings' ? 'active' : ''}`} onClick={() => setActiveTab('settings')} title={t('sidebar.settings')} aria-current={activeTab === 'settings' ? 'page' : undefined}>
            <Settings size={22} />
            <span>{t('sidebar.settings')}</span>
          </button>
        </nav>
      </aside>

      <main className="main-content">
        {activeTab === 'source' && (
          <Source 
            itemsPerPage={wallpapersPerPage}
            onApplyAndGenerate={handleApplyAndGenerate} 
            onSelectForColors={handleSelectForColors} 
            themeColorHex={schemeData?.colors?.primary?.[effectiveMode]?.hex ?? schemeData?.colors?.primary?.[effectiveMode]?.color ?? null}
          />
        )}

        {activeTab === 'presets' && (
          <Presets
            currentWallpaper={wallpaperPath}
            currentSchemeType={schemeType}
            currentSchemeData={schemeData}
            currentMode={mode}
            currentSettings={studioSettings}
            onApplyPreset={handleApplyPreset}
          />
        )}

        {activeTab === 'colors' && (
          <div className="appearance-page page-transition">
            <PageHeader title={t('colors.title')} actions={<div className="appearance-controls">
              <Select label="Mode" disabled={isLoading || !settingsReady} value={mode}
                onValueChange={value => handleModeChange(value as "dark" | "light" | "auto")}
                options={[{ value: "dark", label: "Dark" }, { value: "light", label: "Light" }, { value: "auto", label: "Auto (Wallpaper)" }]} />
              <Select label="Scheme" disabled={isLoading || !settingsReady} value={schemeType} onValueChange={handleSchemeTypeChange}
                options={[
                  ...["Smart", "Content", "Expressive", "Fidelity", "Fruit Salad", "Neutral", "Rainbow", "Tonal Spot", "Vibrant"].map(name => ({ value: `Tinted ${name}`, label: name, group: "Tinted" })),
                  ...["Smart", "Content", "Expressive", "Fidelity", "Fruit Salad", "Monochrome", "Neutral", "Rainbow", "Tonal Spot", "Vibrant"].map(name => ({ value: name, label: name, group: "Classic" })),
                ]} />
            </div>} />

            <div className="colors-workspace-grid">
            {/* Left Column */}
            <div className="card colors-wallpaper-card">
              <h3>{t('colors.wallpaperPreview')}</h3>
              {wallpaperPath ? (
                <button type="button"
                  className="wallpaper-preview" 
                  style={{ 
                    backgroundImage: `url("${convertFileSrc(wallpaperPath)}")`,
                    backgroundSize: 'cover',
                    backgroundPosition: 'center',
                    cursor: 'pointer'
                  }}
                  aria-label={t('colors.wallpaperPreview')}
                  onClick={selectImage}
                />
              ) : (
                <button type="button" className="wallpaper-preview wallpaper-placeholder" onClick={selectImage}>
                  <Image size={48} />
                  <span>{isLoading ? t('colors.generatingPalette') : t('colors.clickToSelect')}</span>
                </button>
              )}
              
              {errorMsg && (
                <div style={{ padding: 12, backgroundColor: 'rgba(255,0,0,0.1)', color: '#ff6b6b', borderRadius: 8, marginTop: 12 }}>
                  <strong>Error:</strong> {errorMsg}
                </div>
              )}
              
              <div className="palette-title">{t('colors.sourceColor')}</div>
              <div className="color-row">
                <div 
                  className="color-swatch large" 
                  style={{ 
                    backgroundColor: schemeData?.source_color_hex || '#333333',
                    color: isLightColor(schemeData?.source_color_hex || '#333333') ? '#000' : '#fff'
                  }}
                >
                  <span style={{opacity: 0.7}}>{t('colors.extracted')}</span>
                  <span style={{fontWeight: 'bold'}}>{(schemeData?.source_color_hex || '—').toUpperCase()}</span>
                </div>
              </div>
              <AdvancedGeneration settings={studioSettings.generation} candidates={schemeData?.source_candidates ?? []}
                selectedIndex={schemeData?.seed_index ?? 0} disabled={!settingsReady || isLoading} onChange={handleGenerationChange} />
            </div>

            {/* Right Column - Colors */}
            <div className="card colors-palette-card">
              <h3>{t('colors.materialYouPalette')}</h3>
              
              {schemeData ? (
                <div className="colors-palette-scroll">
                  <div className="palette-group">
                    <div className="palette-title">Primary</div>
                    <div className="color-row">
                      {renderColorSwatch("primary", "primary", true)}
                      {renderColorSwatch("on-primary", "on_primary")}
                      {renderColorSwatch("container", "primary_container")}
                      {renderColorSwatch("on-container", "on_primary_container")}
                    </div>
                  </div>

                  <div className="palette-group">
                    <div className="palette-title">Secondary</div>
                    <div className="color-row">
                      {renderColorSwatch("secondary", "secondary", true)}
                      {renderColorSwatch("on-secondary", "on_secondary")}
                      {renderColorSwatch("container", "secondary_container")}
                      {renderColorSwatch("on-container", "on_secondary_container")}
                    </div>
                  </div>

                  <div className="palette-group">
                    <div className="palette-title">Tertiary</div>
                    <div className="color-row">
                      {renderColorSwatch("tertiary", "tertiary", true)}
                      {renderColorSwatch("on-tertiary", "on_tertiary")}
                      {renderColorSwatch("container", "tertiary_container")}
                      {renderColorSwatch("on-container", "on_tertiary_container")}
                    </div>
                  </div>

                  <div className="palette-group">
                    <div className="palette-title">Error</div>
                    <div className="color-row">
                      {renderColorSwatch("error", "error", true)}
                      {renderColorSwatch("on-error", "on_error")}
                      {renderColorSwatch("container", "error_container")}
                      {renderColorSwatch("on-container", "on_error_container")}
                    </div>
                  </div>

                  <div className="palette-group">
                    <div className="palette-title">Surface</div>
                    <div className="color-row">
                      {renderColorSwatch("surface", "surface", true)}
                      {renderColorSwatch("on-surface", "on_surface")}
                      {renderColorSwatch("surface-variant", "surface_variant")}
                      {renderColorSwatch("on-variant", "on_surface_variant")}
                    </div>
                  </div>
                </div>
              ) : (
                <div className="empty-state">
                  <Palette size={48} opacity={0.2} />
                  <p>{t('colors.selectImageToGenerate')}</p>
                </div>
              )}
            </div>
              {renderTemplateColorEditor()}
              {renderKdeColorEditor()}
            </div>
          </div>
        )}

        {activeTab === 'templates' && (
          <Templates schemeData={buildThemeContext()} />
        )}

        {activeTab === 'desktop' && (
          <KdeIntegration
            integrationSettings={studioSettings.integrations}
            onIntegrationSettingsChange={async integrations => {
              await persistSettings({ integrations });
              if (schemeData) {
                const warnings = await invoke<string[]>("apply_kde_integrations", { context: buildThemeContext() });
                warnings.forEach(warning => notify(warning, "info"));
              }
            }}
            schemeData={buildThemeContext()}
            themeContext={buildThemeContext()}
            wallpaperPath={wallpaperPath}
            schemeType={schemeType}
            gtkThemeEnabled={gtkThemeEnabled}
            gtkThemeDark={gtkThemeDark}
            onGtkThemeEnabledChange={persistGtkThemeEnabled}
            onGtkThemeDarkChange={persistGtkThemeDark}
            onNotify={notify}
            onApplyKdeColorscheme={applyKdeSchemeWithOverrides}
            onGenerateFromWallpaper={async (path: string) => {
              const generation = ++schemeGenerationRef.current;
              setWallpaperPath(path);
              try {
                setIsLoading(true);
                await invoke("mark_kde_wallpaper_handled", { wallpaperPath: path }).catch(error =>
                  console.warn("Failed to sync KDE watcher state:", error)
                );
                const rawData = await invoke<SchemeData>("generate_scheme_from_image", { imagePath: path, schemeType, settings: studioSettingsRef.current.generation });
                if (schemeGenerationRef.current !== generation) {
                  throw new Error("Color generation was superseded by a newer request.");
                }
                const data = fixMatugenColors(rawData);
                setSchemeData(data);
                setErrorMsg(null);
                const luma = data.wallpaper_luminance ?? data.source_color_luminance ?? 0;
                const targetDark = mode === "auto" ? luma <= 0.5 : mode === "dark";
                return { raw: buildThemeContext(data), kde: buildThemeContext(data), effectiveDark: targetDark };
              } catch (err: unknown) {
                if (schemeGenerationRef.current === generation) setErrorMsg(String(err));
                throw err;
              } finally {
                if (schemeGenerationRef.current === generation) setIsLoading(false);
              }
            }}
          />
        )}

        {activeTab === 'settings' && (
          <div className="settings-page page-transition">
            <PageHeader title={t('settings.title')} description={t('settings.description')} />
            <Section title={t('settings.general')} className="settings-general">
              <SettingRow title={t('settings.language')} description={t('settings.languageDescription')}>
                <Select label={t('settings.language')} value={i18n.language}
                  onValueChange={value => { void i18n.changeLanguage(value); }}
                  options={[{ value: 'pt-BR', label: 'Português (Brasil)' }, { value: 'en', label: 'English' }]} />
              </SettingRow>
              <SettingRow title={t('settings.wallpapersPerPage')} description={t('settings.wallpapersPerPageDescription')}>
                <Select label={t('settings.wallpapersPerPage')} value={String(wallpapersPerPage)}
                  onValueChange={handleWallpapersPerPageChange}
                  options={WALLPAPERS_PER_PAGE_OPTIONS.map(option => ({ value: String(option), label: String(option) }))} />
              </SettingRow>
            </Section>
            <WallpaperLibraryControls />
            <Section title={t('settings.wallhavenTitle')} description={t('settings.wallhavenDescription')}>
              <SettingRow title={t('settings.wallhavenTitle')} description={<button type="button" className="text-link" onClick={() => openUrl('https://wallhaven.cc/settings/account')}>{t('settings.wallhavenGetKey')}</button>}>
                <input type="password" value={wallhavenApiKeyInput}
                  onChange={e => { setWallhavenApiKeyInput(e.target.value); setWallhavenKeyStatus('idle'); }}
                  placeholder={t('settings.wallhavenKeyPlaceholder')} aria-label={t('settings.wallhavenTitle')} />
                <button className="btn btn-secondary" onClick={validateWallhavenKey} disabled={wallhavenKeyStatus === 'checking'}>
                  {wallhavenKeyStatus === 'checking' ? t('settings.wallhavenValidating') : t('settings.wallhavenValidate')}
                </button>
              </SettingRow>
              {wallhavenKeyStatus === 'valid' && <p className="setting-feedback success" role="status">✓ {t('settings.wallhavenKeyValid')}</p>}
              {wallhavenKeyStatus === 'invalid' && <p className="setting-feedback error" role="alert">✗ {t('settings.wallhavenKeyInvalid')}</p>}
            </Section>
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
