import { useState, useEffect } from "react";
import type { StudioSettings } from "../utils/studioSettings";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import { Layers, Trash2, Check, Download, Palette, Monitor, Share2, Upload, Package, Sun, Moon } from "lucide-react";

// ── Types matching the Rust PresetV2 struct ───────────────────────────────────

interface WallpaperInfo {
  filename: string | null;
  original_path: string | null;
  mime_type: string | null;
  sha256: string | null;
}

interface SchemeInfo {
  scheme_type: string;
  mode: string;
  contrast: number | null;
  opacity: number | null;
  source_color_hex: string | null;
  scheme_data: any;
}

interface CustomColor {
  name: string;
  value: string;
  blend: boolean;
}

interface TemplateSnapshot {
  name: string;
  target_app: string | null;
  enabled: boolean;
  input_path: string | null;
  output_path: string | null;
  template_type: string | null;
  content: string | null;
  pre_hook: string | null;
  post_hook: string | null;
}

interface DesktopSettings {
  target_de: string | null;
  apply_wallpaper: boolean;
  apply_kde_colorscheme: boolean;
  apply_templates: boolean;
}

interface PresetV2 {
  version: number;
  name: string;
  created_at: string | null;
  updated_at: string | null;
  app_version: string | null;
  wallpaper: WallpaperInfo | null;
  scheme: SchemeInfo;
  custom_colors: CustomColor[];
  templates: TemplateSnapshot[];
  desktop: DesktopSettings;
}

interface ImportResult {
  preset: PresetV2;
  warnings: string[];
}

// ── Component ─────────────────────────────────────────────────────────────────

interface PresetsProps {
  currentWallpaper: string | null;
  currentSchemeType: string;
  currentSchemeData: any;
  currentMode: string;
  currentSettings: StudioSettings;
  onApplyPreset: (preset: PresetV2) => Promise<void>;
}

export default function Presets({
  currentWallpaper,
  currentSchemeType,
  currentSchemeData,
  currentMode,
  currentSettings,
  onApplyPreset,
}: PresetsProps) {
  const { t } = useTranslation();
  const [presets, setPresets] = useState<PresetV2[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [newPresetName, setNewPresetName] = useState("");
  const [showSaveModal, setShowSaveModal] = useState(false);
  const [importStatus, setImportStatus] = useState("");
  const [installedTemplateCount, setInstalledTemplateCount] = useState(0);

  const loadPresets = async () => {
    try {
      setIsLoading(true);
      const data: PresetV2[] = await invoke("get_presets");
      setPresets(data);
    } catch (e) {
      console.error(e);
      await message(t('presets.loadError', { error: String(e) }), {
        title: t('presets.title'),
        kind: "error",
      });
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadPresets();
  }, []);

  // Load template count when save modal opens.
  useEffect(() => {
    if (!showSaveModal) return;
    invoke<string[]>("get_installed_templates")
      .then((t) => setInstalledTemplateCount(t.length))
      .catch(() => setInstalledTemplateCount(0));
  }, [showSaveModal]);

  const handleSavePreset = async () => {
    if (!newPresetName.trim()) return;
    if (!currentSchemeData) {
      await message(t('presets.noThemeWarning'), {
        title: t('presets.saveModalTitle'),
        kind: "warning",
      });
      return;
    }

    const normalizedName = newPresetName.trim();
    if (presets.some((preset) => preset.name === normalizedName)) {
      const overwrite = await confirm(t('presets.overwriteConfirm', { name: normalizedName }), {
        title: t('presets.replaceTitle'),
        kind: "warning",
      });
      if (!overwrite) return;
    }

    try {
      setIsSaving(true);
      await invoke("save_preset", {
        input: {
          name: normalizedName,
          wallpaper_path: currentWallpaper,
          scheme_type: currentSchemeType,
          mode: currentMode,
          ...(currentSchemeData?.generation_settings ?? currentSettings.generation),
          seed_index: currentSchemeData?.seed_index ?? currentSettings.generation.seed_index,
          opacity: null,
          source_color_hex: currentSchemeData?.source_color_hex ?? null,
          scheme_data: currentSchemeData,
          custom_colors: [],
          desktop: { target_de: "KDE", apply_wallpaper: true, apply_kde_colorscheme: true, apply_templates: true, integrations: currentSettings.integrations },
        },
      });
      await loadPresets();
      setShowSaveModal(false);
      setNewPresetName("");
    } catch (e: unknown) {
      console.error("Save preset failed:", e);
      await message(t('presets.saveError', { error: String(e) }), {
        title: t('presets.saveModalTitle'),
        kind: "error",
      });
    } finally {
      setIsSaving(false);
    }
  };

  const handleDeletePreset = async (name: string) => {
    try {
      const confirmed = await confirm(t('presets.deleteConfirm', { name }), {
        title: t('presets.deleteTitle'),
        kind: "warning",
      });
      if (!confirmed) return;
      await invoke("delete_preset", { name });
      await loadPresets();
    } catch (e) {
      console.error("Delete preset failed:", e);
      await message(t('presets.deleteError', { error: String(e) }), {
        title: t('presets.deleteTitle'),
        kind: "error",
      });
    }
  };

  const handleExportPreset = async (preset: PresetV2) => {
    try {
      const filePath = await save({
        defaultPath: `${preset.name.replace(/\s+/g, "_")}.matugen`,
        filters: [{ name: "Matugen Preset", extensions: ["matugen"] }],
      });
      if (!filePath) return;

      await invoke("export_preset", { name: preset.name, outputPath: filePath });
      await message(t('presets.exportSuccess', { name: preset.name, path: filePath }), {
        title: t('presets.exportTitle'),
        kind: "info",
      });
    } catch (e) {
      await message(t('presets.exportError', { error: String(e) }), {
        title: t('presets.exportTitle'),
        kind: "error",
      });
    }
  };

  const handleImportPreset = async () => {
    try {
      const filePath = await open({
        multiple: false,
        filters: [{ name: "Matugen Preset", extensions: ["matugen"] }],
      });
      if (!filePath) return;

      setImportStatus("Importing preset...");
      const result: ImportResult = await invoke("import_preset", { filePath });
      await loadPresets();
      setImportStatus("");

      let msg = t('presets.importSuccess', { name: result.preset.name });
      if (result.warnings.length > 0) {
        msg += "\n\n" + t('presets.importWarnings') + "\n• " + result.warnings.join("\n• ");
      }
      await message(msg, {
        title: t('presets.import'),
        kind: result.warnings.length > 0 ? "warning" : "info",
      });
    } catch (e) {
      setImportStatus("");
      await message(t('presets.importError', { error: String(e) }), {
        title: t('presets.import'),
        kind: "error",
      });
    }
  };

  // Extract a colour for preview from scheme_data.
  const getColor = (schemeData: any, name: string): string | null => {
    const c = schemeData?.colors?.[name];
    if (!c) return null;
    for (const v of ["default", "dark", "light"]) {
      const raw = c[v]?.color || c[v]?.hex;
      if (raw) return raw.length === 9 && raw.startsWith("#") ? "#" + raw.slice(1, 7) : raw;
    }
    return null;
  };

  const modeIcon = (m: string) =>
    m === "light" ? <Sun size={12} /> : <Moon size={12} />;

  return (
    <div className="tab-content presets-page page-transition">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div>
          <h2>{t('presets.title')}</h2>
          <p style={{ color: "var(--text-secondary)" }}>{t('presets.subtitle')}</p>
        </div>

        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-secondary" onClick={handleImportPreset}>
            <Upload size={16} />
            {t('presets.import')}
          </button>
          <button className="btn btn-primary" onClick={() => setShowSaveModal(true)} disabled={!currentSchemeData}>
            <Download size={18} />
            {t('presets.saveCurrent')}
          </button>
        </div>
      </div>

      {importStatus && (
        <div style={{
          padding: 12,
          borderRadius: 8,
          background: "rgba(74, 222, 128, 0.1)",
          color: "#4ade80",
          fontSize: 13,
          display: "flex",
          alignItems: "center",
          gap: 8,
        }}>
          <span className="pulse-dot" />
          {importStatus}
        </div>
      )}

      <div className="collection-grid">
        {isLoading ? (
          <p>{t('presets.loading')}</p>
        ) : presets.length === 0 ? (
          <div className="empty-state collection-empty">
            <Layers size={48} opacity={0.2} />
            <strong>{t('presets.noPresets')}</strong>
            <p style={{ fontSize: 13, color: "var(--text-secondary)" }}>
              {t('presets.saveOrImport')}
            </p>
            <button className="btn btn-secondary btn-compact" onClick={handleImportPreset}>{t('presets.import')}</button>
          </div>
        ) : (
          presets.map((preset, idx) => {
            const sd = preset.scheme.scheme_data;
            const primary   = getColor(sd, "primary");
            const secondary = getColor(sd, "secondary");
            const tertiary  = getColor(sd, "tertiary");
            const surface   = getColor(sd, "surface");

            return (
              <div key={idx} className="template-card preset-card">
                {/* Colour band preview */}
                <div style={{ display: "flex", height: 8, borderRadius: 4, overflow: "hidden", gap: 2 }}>
                  <div style={{ flex: 3, background: primary   || "var(--accent)" }} />
                  <div style={{ flex: 2, background: secondary || "var(--border)" }} />
                  <div style={{ flex: 2, background: tertiary  || "var(--surface-hover)" }} />
                  <div style={{ flex: 1, background: surface   || "var(--surface)" }} />
                </div>

                <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                  <div style={{
                    background: primary || "var(--accent-transparent)",
                    width: 44, height: 44, borderRadius: 12,
                    display: "flex", alignItems: "center", justifyContent: "center",
                    color: "#fff", boxShadow: "0 4px 12px rgba(0,0,0,0.15)", flexShrink: 0,
                  }}>
                    <Palette size={22} />
                  </div>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <h3 style={{ fontSize: 16, margin: 0, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                      {preset.name}
                    </h3>
                    <span style={{ fontSize: 12, color: "var(--text-secondary)", textTransform: "capitalize" }}>
                      {preset.scheme.scheme_type} · {modeIcon(preset.scheme.mode)} {preset.scheme.mode}
                    </span>
                  </div>
                </div>

                <div style={{ fontSize: 12, color: "var(--text-secondary)", display: "flex", alignItems: "center", gap: 6 }}>
                  <Monitor size={13} />
                  <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {preset.wallpaper?.filename ?? t('presets.noWallpaper')}
                  </span>
                </div>

                {preset.templates.length > 0 && (
                  <div style={{ fontSize: 12, color: "var(--text-secondary)", display: "flex", alignItems: "center", gap: 6 }}>
                    <Package size={13} />
                    <span>{preset.templates.length === 1 ? t('presets.templateCount_one', { count: 1 }) : t('presets.templateCount_other', { count: preset.templates.length })}</span>
                  </div>
                )}

                <div style={{ display: "flex", gap: 6, marginTop: "auto" }}>
                  <button
                    className="btn btn-primary btn-compact"
                    style={{ flex: 1 }}
                    onClick={() => onApplyPreset(preset)}
                  >
                    <Check size={15} /> {t('presets.apply')}
                  </button>
                  <button
                    className="btn btn-secondary icon-btn"
                    onClick={() => handleExportPreset(preset)}
                    title={t('presets.exportTitle')}
                    aria-label={`Export ${preset.name}`}
                  >
                    <Share2 size={15} />
                  </button>
                  <button
                    className="btn btn-danger icon-btn"
                    onClick={() => handleDeletePreset(preset.name)}
                    title={t('presets.deleteTitle')}
                    aria-label={`Delete ${preset.name}`}
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>

      {showSaveModal && (
        <div className="modal-overlay">
          <div className="modal-content" role="dialog" aria-modal="true" aria-label="Save Preset" style={{ width: 'min(92vw, 420px)', maxWidth: 420 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
              <h3>Save Preset</h3>
              <button className="btn btn-ghost icon-btn" onClick={() => setShowSaveModal(false)} title="Close" aria-label="Close save preset dialog">✕</button>
            </div>

            <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
              <label style={{ fontSize: 14, color: "var(--text-secondary)" }}>Preset Name</label>
              <input
                type="text"
                value={newPresetName}
                onChange={(e) => setNewPresetName(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleSavePreset()}
                placeholder="e.g. Cyberpunk Red"
                style={{
                  padding: "12px",
                  borderRadius: 8,
                  border: "1px solid var(--border)",
                  background: "var(--surface-hover)",
                  color: "var(--text-primary)",
                  fontSize: 16,
                }}
              />

              {/* Snapshot summary */}
              <div style={{
                padding: 12,
                borderRadius: 8,
                background: "var(--surface-hover)",
                fontSize: 12,
                color: "var(--text-secondary)",
                display: "flex",
                flexDirection: "column",
                gap: 6,
              }}>
                <div style={{ display: "flex", alignItems: "center", gap: 6, fontWeight: 600, color: "var(--text-primary)" }}>
                  <Package size={13} />
                  <span>This preset will include:</span>
                </div>
                <div style={{ paddingLeft: 4, display: "flex", flexDirection: "column", gap: 3 }}>
                  <span>• Scheme: <strong>{currentSchemeType}</strong> · {currentMode}</span>
                  <span>• Wallpaper: <strong>{currentWallpaper ? currentWallpaper.split("/").pop() : "none"}</strong></span>
                  <span>• Templates: <strong>{installedTemplateCount}</strong> installed</span>
                  {!currentWallpaper && (
                    <span style={{ color: "#facc15" }}>
                      ⚠ No wallpaper selected — colour-only preset
                    </span>
                  )}
                </div>
              </div>

              <div style={{ marginTop: 8, display: "flex", gap: 12 }}>
                <button className="btn btn-secondary" style={{ flex: 1 }} onClick={() => setShowSaveModal(false)}>
                  Cancel
                </button>
                <button
                  className="btn btn-primary"
                  style={{ flex: 1 }}
                  disabled={!newPresetName.trim() || isSaving}
                  onClick={handleSavePreset}
                >
                  {isSaving ? "Saving..." : "Save"}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
