import { useEffect, useMemo, useState } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Image, Palette, LayoutTemplate, Monitor, Settings, Download, Copy, Edit2, Check, X, RotateCcw } from "lucide-react";
import Wheel from '@uiw/react-color-wheel';
import ShadeSlider from '@uiw/react-color-shade-slider';
import { hsvaToHex, hexToHsva, hexToRgba } from '@uiw/color-convert';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import Templates from "./pages/Templates";
import Source from "./pages/Source";
import Presets from "./pages/Presets";
import KdeIntegration from "./pages/KdeIntegration";
import {
  DEFAULT_WALLPAPERS_PER_PAGE,
  WALLPAPERS_PER_PAGE_OPTIONS,
  validateWallpapersPerPage,
} from "./utils/wallpaperPagination";
import "./App.css";

const WALLPAPERS_PER_PAGE_STORAGE_KEY = "wallpapersPerPage";
const KDE_COLOR_OVERRIDES_STORAGE_KEY = "kdeColorOverrides";
const GTK_THEME_ENABLED_STORAGE_KEY = "gtkThemeEnabled";
const GTK_THEME_DARK_STORAGE_KEY = "gtkThemeDark";

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

function App() {
  const [activeTab, setActiveTab] = useState("colors");
  const [wallpaperPath, setWallpaperPath] = useState<string | null>(null);
  const [schemeData, setSchemeData] = useState<any>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [schemeType, setSchemeType] = useState("Content");
  const [mode, setMode] = useState<"dark" | "light" | "system">("dark");
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
  const [wallpapersPerPage, setWallpapersPerPage] = useState(() => {
    const saved = localStorage.getItem(WALLPAPERS_PER_PAGE_STORAGE_KEY);
    const value = validateWallpapersPerPage(saved ?? DEFAULT_WALLPAPERS_PER_PAGE);
    localStorage.setItem(WALLPAPERS_PER_PAGE_STORAGE_KEY, String(value));
    return value;
  });

  const handleWallpapersPerPageChange = (value: string) => {
    const parsed = validateWallpapersPerPage(value);
    setWallpapersPerPage(parsed);
    localStorage.setItem(WALLPAPERS_PER_PAGE_STORAGE_KEY, String(parsed));
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

  const writeSchemeColor = (context: any, path: string, hex: string) => {
    if (!context.colors) context.colors = {};
    if (!context.colors[path]) context.colors[path] = {};
    for (const variant of ["default", "dark", "light"]) {
      context.colors[path][variant] = {
        ...(context.colors[path][variant] ?? {}),
        hex,
        color: hex,
      };
    }
  };

  const buildKdeSchemeContext = (baseContext: any = schemeData, overrides: Record<string, string> = kdeColorOverrides) => {
    if (!baseContext) return baseContext;
    const context = JSON.parse(JSON.stringify(baseContext));
    KDE_COLOR_CONTROLS.forEach(control => {
      const overrideHex = normalizeHex(overrides[control.key]);
      if (overrideHex) {
        writeSchemeColor(context, control.path, overrideHex);
      }
    });
    return context;
  };

  const saveKdeColorOverrides = (next: Record<string, string>) => {
    setKdeColorOverrides(next);
    localStorage.setItem(KDE_COLOR_OVERRIDES_STORAGE_KEY, JSON.stringify(next));
  };

  const getKdeControlColor = (control: KdeColorControl, overrides: Record<string, string> = kdeColorOverrides, data: any = schemeData) => {
    return normalizeHex(overrides[control.key]) || normalizeHex(kdeDiskColors[control.key]) || readSchemeColor(data, control.path, control.fallback);
  };

  const persistGtkThemeEnabled = (enabled: boolean) => {
    setGtkThemeEnabled(enabled);
    localStorage.setItem(GTK_THEME_ENABLED_STORAGE_KEY, String(enabled));
  };

  const persistGtkThemeDark = (dark: boolean) => {
    setGtkThemeDark(dark);
    localStorage.setItem(GTK_THEME_DARK_STORAGE_KEY, String(dark));
  };

  const selectImage = async () => {
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
        setWallpaperPath(path);
        setIsLoading(true);
        
        // Invoke backend command
        const data = await invoke("generate_scheme_from_image", { imagePath: path, schemeType });
        console.log("Data received:", data);
        setSchemeData(data);
        setErrorMsg(null);
      }
    } catch (err: any) {
      console.error("Failed to select image or generate scheme", err);
      setErrorMsg(err.toString());
    } finally {
      setIsLoading(false);
    }
  };

  // Inject .color to all color groups so Matugen parses them correctly
  const fixMatugenColors = (data: any) => {
    if (!data || !data.colors) return data;
    const cloned = JSON.parse(JSON.stringify(data));
    for (const key in cloned.colors) {
      const group = cloned.colors[key];
      if (group.dark && group.dark.hex) group.dark.color = group.dark.hex;
      if (group.light && group.light.hex) group.light.color = group.light.hex;
      if (group.default && group.default.hex) group.default.color = group.default.hex;
    }
    return cloned;
  };

  const handleSchemeTypeChange = async (newType: string) => {
    setSchemeType(newType);
    if (wallpaperPath) {
      try {
        setIsLoading(true);
        const rawData = await invoke("generate_scheme_from_image", { imagePath: wallpaperPath, schemeType: newType });
        const data = fixMatugenColors(rawData);
        setSchemeData(data);
        setErrorMsg(null);
      } catch (err: any) {
        setErrorMsg(err.toString());
      } finally {
        setIsLoading(false);
      }
    }
  };

  const handleApplyAndGenerate = async (path: string) => {
    setWallpaperPath(path);
    try {
      setIsLoading(true);
      await invoke("mark_kde_wallpaper_handled", { wallpaperPath: path }).catch(e =>
        console.warn("Failed to sync KDE watcher state:", e)
      );
      const rawData = await invoke("generate_scheme_from_image", { imagePath: path, schemeType });
      const data = fixMatugenColors(rawData);
      setSchemeData(data);
      setErrorMsg(null);
      const tasks: Promise<any>[] = [
        invoke("apply_theme", { context: data }),
        invoke("apply_kde_colorscheme", { context: buildKdeSchemeContext(data) }),
      ];
      if (gtkThemeEnabled) {
        tasks.push(invoke("apply_gtk_theme", { context: data, dark: gtkThemeDark }));
      }
      const results = await Promise.allSettled(tasks);
      const failed = results.find(result => result.status === "rejected");
      if (failed && failed.status === "rejected") {
        throw failed.reason;
      }
      await loadTemplateColorControls(data);
      await loadKdeDiskColors();
      alert("Theme generated and applied globally!");
    } catch (err: any) {
      setErrorMsg(err.toString());
      alert("Failed to generate or apply theme: " + err);
    } finally {
      setIsLoading(false);
    }
  };


  const handleSelectForColors = async (path: string) => {
    setWallpaperPath(path);
    setActiveTab('colors');
    try {
      setIsLoading(true);
      const rawData = await invoke("generate_scheme_from_image", { imagePath: path, schemeType });
      const data = fixMatugenColors(rawData);
      setSchemeData(data);
      setErrorMsg(null);
    } catch (err: any) {
      setErrorMsg(err.toString());
      alert("Failed to generate colors: " + err);
    } finally {
      setIsLoading(false);
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
      await invoke("apply_theme", { context: schemeData });
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
      await invoke("apply_theme", { context: schemeData });
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
      await invoke("apply_kde_colorscheme", { context: buildKdeSchemeContext(schemeData, next) });
    } else {
      await invoke("apply_kde_color_values", { values: Object.entries(next).map(([key, hex]) => ({ key, hex })) });
      await loadKdeDiskColors();
    }
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
      await invoke("apply_kde_colorscheme", { context: buildKdeSchemeContext(schemeData, next) });
    } else {
      await invoke("apply_kde_color_values", { values: Object.entries(next).map(([key, hex]) => ({ key, hex })) });
      await loadKdeDiskColors();
    }
  };

  const handleApplyPreset = async (preset: any) => {
    const wallpaper = preset.wallpaper?.original_path ?? null;
    const scheme_data = preset.scheme.scheme_data;
    const scheme_type = preset.scheme.scheme_type;

    setWallpaperPath(wallpaper);
    setSchemeType(scheme_type);
    setSchemeData(scheme_data);
    setMode((preset.scheme.mode ?? "dark") as "dark" | "light" | "system");

    const desktop = preset.desktop ?? { apply_wallpaper: true, apply_kde_colorscheme: true, apply_templates: true };

    try {
      if (wallpaper && desktop.apply_wallpaper) {
        await invoke("apply_wallpaper", { imagePath: wallpaper, screenIndex: -1 });
      }
      if (desktop.apply_templates) {
        await invoke("apply_theme", { context: scheme_data });
      }
      if (desktop.apply_kde_colorscheme) {
        invoke("apply_kde_colorscheme", { context: buildKdeSchemeContext(scheme_data) }).catch(e =>
          console.error("KDE scheme apply failed:", e)
        );
      }
      if (gtkThemeEnabled) {
        invoke("apply_gtk_theme", { context: scheme_data, dark: gtkThemeDark }).catch(e =>
          console.error("GTK theme apply failed:", e)
        );
      }
      alert("Preset applied!");
    } catch (e) {
      alert("Failed to apply preset: " + e);
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
      hex = schemeData.colors[path].dark.hex || schemeData.colors[path].dark.color || "#333333";
    }
    // Calculate contrast text color
    const textColor = isLightColor(hex) ? '#000000' : '#ffffff';

    return (
      <div 
        className={`color-swatch ${isLarge ? 'large' : ''}`} 
        style={{ backgroundColor: hex, color: textColor }}
        title={name}
        onClick={() => { 
          setSelectedColor({name, path, hex, originalHex: hex, scope: "matugen"});
          setPickerHsva(hexToHsva(hex));
          setShowPicker(false); 
        }}
      >
        <span style={{opacity: 0.7}}>{name}</span>
        <span style={{fontWeight: 'bold'}}>{hex.toUpperCase()}</span>
      </div>
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
              void resetTemplateOverride(group.templateName, control.key).catch(e => alert("Failed to reset override: " + e));
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.stopPropagation();
                void resetTemplateOverride(group.templateName, control.key).catch(e => alert("Failed to reset override: " + e));
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
            <h3>Template Color Overrides</h3>
            <p>Adjust the colors each installed template exposes, without changing the Material You palette.</p>
          </div>
          <div className="template-colors-summary">
            <span>{templateColorGroups.length} templates</span>
            <span>{templateOverrideCount} overrides</span>
          </div>
        </div>

        {isLoadingTemplateColors ? (
          <div className="template-colors-empty">
            <Palette size={28} opacity={0.35} />
            <span>Scanning installed templates...</span>
          </div>
        ) : templateColorGroups.length === 0 ? (
          <div className="template-colors-empty">
            <LayoutTemplate size={28} opacity={0.35} />
            <span>Install templates to edit their exposed color tokens here.</span>
          </div>
        ) : (
          <>
            <div className="template-colors-toolbar">
              <select value={activeTemplateGroup} onChange={(e) => setActiveTemplateGroup(e.target.value)}>
                {templateColorGroups.map(group => (
                  <option key={group.templateName} value={group.templateName}>
                    {group.displayName} · {group.controls.length}
                  </option>
                ))}
              </select>
              {selectedTemplateGroup && (
                <button
                  type="button"
                  className="btn btn-secondary btn-compact"
                  disabled={!selectedTemplateGroup.controls.some(control => control.overridden)}
                  onClick={() => resetTemplateOverride(selectedTemplateGroup.templateName).catch(e => alert("Failed to reset overrides: " + e))}
                >
                  <RotateCcw size={14} />
                  Reset template
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
            onClick={() => resetKdeColorOverride().catch(e => alert("Failed to reset KDE colors: " + e))}
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
                        void resetKdeColorOverride(control.key).catch(e => alert("Failed to reset KDE color: " + e));
                      }}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.stopPropagation();
                          void resetKdeColorOverride(control.key).catch(e => alert("Failed to reset KDE color: " + e));
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
      {selectedColor && (
        <div className="modal-overlay" onClick={() => { setSelectedColor(null); setShowPicker(false); }}>
          <div className="modal-content" onClick={e => e.stopPropagation()}>
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
              <X size={20} cursor="pointer" onClick={() => { setSelectedColor(null); setShowPicker(false); }} />
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
                          alert("Generate a color palette first!");
                          return;
                        }
                        await invoke("apply_theme", { context: schemeData });
                      }
                      setSelectedColor(null);
                      setShowPicker(false);
                    } catch (e) {
                      console.error(e);
                      alert("Failed to apply color change: " + e);
                    }
                  }}
                >
                  <Check size={16} /> {selectedColor.scope === "template" || selectedColor.scope === "kde" ? "Save Override" : "Apply Changes to System"}
                </button>
              </div>
            )}
          </div>
        </div>
      )}

      <aside className="sidebar">
        <div className="sidebar-logo" style={{ color: "var(--accent)", marginBottom: 20 }}>
          <Palette size={32} />
        </div>
        <div className={`sidebar-item ${activeTab === 'source' ? 'active' : ''}`} onClick={() => setActiveTab('source')}>
          <Image size={24} />
          <span>Source</span>
        </div>
        <div className={`sidebar-item ${activeTab === 'presets' ? 'active' : ''}`} onClick={() => setActiveTab('presets')}>
          <Download size={24} />
          <span>Presets</span>
        </div>
        <div className={`sidebar-item ${activeTab === 'colors' ? 'active' : ''}`} onClick={() => setActiveTab('colors')}>
          <Palette size={24} />
          <span>Colors</span>
        </div>
        <div className={`sidebar-item ${activeTab === 'templates' ? 'active' : ''}`} onClick={() => setActiveTab('templates')}>
          <LayoutTemplate size={24} />
          <span>Templates</span>
        </div>
        <div className={`sidebar-item ${activeTab === 'desktop' ? 'active' : ''}`} onClick={() => setActiveTab('desktop')}>
          <Monitor size={24} />
          <span>Desktop</span>
        </div>
        <div style={{ flex: 1 }}></div>
        <div className="sidebar-item" style={{ cursor: 'pointer', background: 'var(--accent-transparent)', color: 'var(--accent)' }} onClick={async () => {
          if (!schemeData) {
            alert("Generate a color palette first!");
            return;
          }
          try {
            const tasks: Promise<any>[] = [
              invoke("apply_theme", { context: schemeData }),
              invoke("apply_kde_colorscheme", { context: buildKdeSchemeContext(schemeData) }),
            ];
            if (gtkThemeEnabled) {
              tasks.push(invoke("apply_gtk_theme", { context: schemeData, dark: gtkThemeDark }));
            }
            const results = await Promise.allSettled(tasks);
            const failed = results.find(result => result.status === "rejected");
            if (failed && failed.status === "rejected") {
              throw failed.reason;
            }
            alert("Theme applied globally to all installed templates!");
          } catch (e) {
            alert("Failed to apply theme: " + e);
          }
        }}>
          <Download size={24} />
          <span style={{ fontWeight: 'bold' }}>Apply</span>
        </div>
        <div className={`sidebar-item ${activeTab === 'settings' ? 'active' : ''}`} onClick={() => setActiveTab('settings')}>
          <Settings size={24} />
          <span>Settings</span>
        </div>
      </aside>

      <main className="main-content">
        {activeTab === 'source' && (
          <Source 
            itemsPerPage={wallpapersPerPage}
            onApplyAndGenerate={handleApplyAndGenerate} 
            onSelectForColors={handleSelectForColors} 
          />
        )}

        {activeTab === 'presets' && (
          <Presets
            currentWallpaper={wallpaperPath}
            currentSchemeType={schemeType}
            currentSchemeData={schemeData}
            currentMode={mode}
            onApplyPreset={handleApplyPreset}
          />
        )}

        {activeTab === 'colors' && (
          <>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <h2>Matugen Studio</h2>
              <div className="controls-row">
                <select value={mode} onChange={(e) => setMode(e.target.value as "dark" | "light" | "system")}>
                  <option value="dark">Dark</option>
                  <option value="light">Light</option>
                  <option value="system">System</option>
                </select>
                <select value={schemeType} onChange={(e) => handleSchemeTypeChange(e.target.value)}>
                  <option>Content</option>
                  <option>Expressive</option>
                  <option>Fidelity</option>
                  <option>Fruit Salad</option>
                  <option>Monochrome</option>
                  <option>Neutral</option>
                  <option>Rainbow</option>
                  <option>Tonal Spot</option>
                  <option>Vibrant</option>
                </select>
              </div>
            </div>

            <div className="colors-workspace-grid">
            {/* Left Column */}
            <div className="card colors-wallpaper-card">
              <h3>Wallpaper Preview</h3>
              {wallpaperPath ? (
                <div 
                  className="wallpaper-preview" 
                  style={{ 
                    backgroundImage: `url("${convertFileSrc(wallpaperPath)}")`,
                    backgroundSize: 'cover',
                    backgroundPosition: 'center',
                    cursor: 'pointer'
                  }}
                  onClick={selectImage}
                />
              ) : (
                <div className="wallpaper-preview wallpaper-placeholder" onClick={selectImage}>
                  <Image size={48} />
                  <span>{isLoading ? "Generating palette..." : "Click to select an image"}</span>
                </div>
              )}
              
              {errorMsg && (
                <div style={{ padding: 12, backgroundColor: 'rgba(255,0,0,0.1)', color: '#ff6b6b', borderRadius: 8, marginTop: 12 }}>
                  <strong>Error:</strong> {errorMsg}
                </div>
              )}
              
              <div className="palette-title">Source Color</div>
              <div className="color-row">
                <div 
                  className="color-swatch large" 
                  style={{ 
                    backgroundColor: schemeData?.source_color_hex || '#333333',
                    color: isLightColor(schemeData?.source_color_hex || '#333333') ? '#000' : '#fff'
                  }}
                >
                  <span style={{opacity: 0.7}}>Extracted</span>
                  <span style={{fontWeight: 'bold'}}>{(schemeData?.source_color_hex || '#xxxxxx').toUpperCase()}</span>
                </div>
              </div>
            </div>

            {/* Right Column - Colors */}
            <div className="card colors-palette-card">
              <h3>Material You Palette</h3>
              
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
                  <p>Select an image to generate colors</p>
                </div>
              )}
            </div>
              {renderTemplateColorEditor()}
              {renderKdeColorEditor()}
            </div>
          </>
        )}

        {activeTab === 'templates' && (
          <Templates schemeData={schemeData} />
        )}

        {activeTab === 'desktop' && (
          <KdeIntegration 
            schemeData={buildKdeSchemeContext()}
            wallpaperPath={wallpaperPath}
            schemeType={schemeType}
            gtkThemeEnabled={gtkThemeEnabled}
            gtkThemeDark={gtkThemeDark}
            onGtkThemeEnabledChange={persistGtkThemeEnabled}
            onGtkThemeDarkChange={persistGtkThemeDark}
            onGenerateFromWallpaper={async (path: string) => {
              setWallpaperPath(path);
              try {
                setIsLoading(true);
                const rawData = await invoke("generate_scheme_from_image", { imagePath: path, schemeType });
                const data = fixMatugenColors(rawData);
                setSchemeData(data);
                setErrorMsg(null);
                return data;
              } catch (err: any) {
                setErrorMsg(err.toString());
                throw err;
              } finally {
                setIsLoading(false);
              }
            }}
          />
        )}

        {activeTab === 'settings' && (
          <div style={{ padding: 24, display: 'flex', flexDirection: 'column', gap: 24, height: '100%', width: '100%' }}>
            <div>
              <h2>Settings</h2>
              <p style={{ color: 'var(--text-secondary)', fontSize: 13 }}>
                Ajustes locais de desempenho e comportamento do app.
              </p>
            </div>

            <div className="card" style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', gap: 16, alignItems: 'flex-start', flexWrap: 'wrap' }}>
                <div style={{ maxWidth: 620 }}>
                  <h3 style={{ marginTop: 0 }}>Wallpapers por página</h3>
                  <p style={{ color: 'var(--text-secondary)', fontSize: 13, marginBottom: 0 }}>
                    Controla quantos wallpapers são carregados por vez nas sources. Valores menores reduzem uso de memória e melhoram desempenho em pastas grandes.
                  </p>
                </div>
                <select
                  value={wallpapersPerPage}
                  onChange={(e) => handleWallpapersPerPageChange(e.target.value)}
                  style={{ minWidth: 120, padding: '10px 12px', borderRadius: 8, background: 'var(--surface-hover)', border: '1px solid var(--border)', color: 'var(--text-primary)' }}
                >
                  {WALLPAPERS_PER_PAGE_OPTIONS.map(option => (
                    <option key={option} value={option}>{option}</option>
                  ))}
                </select>
              </div>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
