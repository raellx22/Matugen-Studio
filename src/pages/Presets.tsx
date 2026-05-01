import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Layers, Trash2, Check, Download, Palette, Monitor, Share2, Upload, Package } from "lucide-react";

interface Preset {
  name: string;
  wallpaper_path: string | null;
  scheme_type: string;
  scheme_data: any;
}

interface PresetsProps {
  currentWallpaper: string | null;
  currentSchemeType: string;
  currentSchemeData: any;
  onApplyPreset: (wallpaper: string | null, schemeType: string, schemeData: any) => Promise<void>;
}

export default function Presets({ currentWallpaper, currentSchemeType, currentSchemeData, onApplyPreset }: PresetsProps) {
  const [presets, setPresets] = useState<Preset[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [newPresetName, setNewPresetName] = useState("");
  const [showSaveModal, setShowSaveModal] = useState(false);
  const [importStatus, setImportStatus] = useState("");

  const loadPresets = async () => {
    try {
      setIsLoading(true);
      const data: Preset[] = await invoke("get_presets");
      setPresets(data);
    } catch (e) {
      console.error(e);
      alert("Failed to load presets: " + e);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadPresets();
  }, []);

  const handleSavePreset = async () => {
    if (!newPresetName.trim()) return;
    if (!currentSchemeData) {
      alert("No theme generated yet. Please go to Source tab and apply a wallpaper first.");
      return;
    }

    try {
      setIsSaving(true);
      const newPreset: Preset = {
        name: newPresetName.trim(),
        wallpaper_path: currentWallpaper,
        scheme_type: currentSchemeType,
        scheme_data: currentSchemeData
      };

      console.log("Saving preset:", newPreset);

      await invoke("save_preset", { preset: newPreset });
      console.log("save_preset invoke succeeded");
      await loadPresets();
      setShowSaveModal(false);
      setNewPresetName("");
    } catch (e: any) {
      console.error("Save preset failed:", e);
      alert("Failed to save preset: " + e);
    } finally {
      setIsSaving(false);
    }
  };
  const handleDeletePreset = async (name: string) => {
    if (confirm(`Are you sure you want to delete the preset '${name}'?`)) {
      try {
        await invoke("delete_preset", { name });
        await loadPresets();
      } catch (e) {
        alert("Failed to delete preset: " + e);
      }
    }
  };

  const handleExportPreset = async (preset: Preset) => {
    try {
      // Get installed templates to bundle with the preset
      const installedTemplates: string[] = await invoke("get_installed_templates");
      
      // Open a save dialog with .matugen extension
      const filePath = await save({
        defaultPath: `${preset.name.replace(/\s+/g, '_')}.matugen`,
        filters: [{
          name: "Matugen Preset",
          extensions: ["matugen"]
        }]
      });

      if (!filePath) return;

      await invoke("export_preset", {
        preset,
        installedTemplates,
        outputPath: filePath
      });

      alert(`Preset "${preset.name}" exported successfully!\n\nFile: ${filePath}`);
    } catch (e) {
      alert("Failed to export preset: " + e);
    }
  };

  const handleImportPreset = async () => {
    try {
      const filePath = await open({
        multiple: false,
        filters: [{
          name: "Matugen Preset",
          extensions: ["matugen"]
        }]
      });

      if (!filePath) return;

      setImportStatus("Importing preset...");
      
      const imported: Preset = await invoke("import_preset", { filePath });
      
      await loadPresets();
      setImportStatus("");
      alert(`Preset "${imported.name}" imported successfully!\n\nWallpaper and colors are ready to apply.`);
    } catch (e) {
      setImportStatus("");
      alert("Failed to import preset: " + e);
    }
  };

  // Helper to get a color for preview from scheme data
  const getColor = (data: any, name: string) => {
    const c = data?.colors?.[name];
    if (!c) return null;
    for (const v of ['default', 'dark', 'light']) {
      const raw = c[v]?.color || c[v]?.hex;
      if (raw) return raw.length === 9 && raw.startsWith('#') ? '#' + raw.slice(1, 7) : raw;
    }
    return null;
  };

  return (
    <div className="tab-content" style={{ display: 'flex', flexDirection: 'column', gap: 24, height: '100%' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h2>Theme Presets</h2>
          <p style={{ color: 'var(--text-secondary)' }}>Save, share and manage your favorite setups</p>
        </div>
        
        <div style={{ display: 'flex', gap: 8 }}>
          <button className="btn btn-secondary" onClick={handleImportPreset} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <Upload size={16} />
            Import .matugen
          </button>
          <button className="btn btn-primary" onClick={() => setShowSaveModal(true)} disabled={!currentSchemeData}>
            <Download size={18} />
            Save Current Setup
          </button>
        </div>
      </div>

      {importStatus && (
        <div style={{
          padding: 12,
          borderRadius: 8,
          background: 'rgba(74, 222, 128, 0.1)',
          color: '#4ade80',
          fontSize: 13,
          display: 'flex',
          alignItems: 'center',
          gap: 8
        }}>
          <span className="pulse-dot" />
          {importStatus}
        </div>
      )}

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(300px, 1fr))', gap: 16, overflowY: 'auto' }}>
        {isLoading ? (
          <p>Loading presets...</p>
        ) : presets.length === 0 ? (
          <div className="empty-state" style={{ gridColumn: '1 / -1' }}>
            <Layers size={48} opacity={0.2} />
            <p>No presets saved yet.</p>
            <p style={{ fontSize: 13, color: 'var(--text-secondary)' }}>
              Save your current setup or import a .matugen file from a friend!
            </p>
          </div>
        ) : (
          presets.map((preset, idx) => {
            const primary = getColor(preset.scheme_data, "primary");
            const secondary = getColor(preset.scheme_data, "secondary");
            const tertiary = getColor(preset.scheme_data, "tertiary");
            const surface = getColor(preset.scheme_data, "surface");
            
            return (
              <div key={idx} className="template-card" style={{
                background: 'var(--surface)',
                borderRadius: 16,
                padding: 20,
                border: '1px solid var(--border)',
                display: 'flex',
                flexDirection: 'column',
                gap: 12,
                transition: 'all 0.2s'
              }}>
                {/* Color band preview */}
                <div style={{ 
                  display: 'flex', 
                  height: 8, 
                  borderRadius: 4, 
                  overflow: 'hidden',
                  gap: 2
                }}>
                  <div style={{ flex: 3, background: primary || 'var(--accent)' }} />
                  <div style={{ flex: 2, background: secondary || 'var(--border)' }} />
                  <div style={{ flex: 2, background: tertiary || 'var(--surface-hover)' }} />
                  <div style={{ flex: 1, background: surface || 'var(--surface)' }} />
                </div>

                <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                  <div style={{ 
                    background: primary || 'var(--accent-transparent)', 
                    width: 44,
                    height: 44,
                    borderRadius: 12,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    color: '#fff',
                    boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
                    flexShrink: 0
                  }}>
                    <Palette size={22} />
                  </div>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <h3 style={{ fontSize: 16, margin: 0, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{preset.name}</h3>
                    <span style={{ fontSize: 12, color: 'var(--text-secondary)', textTransform: 'capitalize' }}>
                      {preset.scheme_type} Scheme
                    </span>
                  </div>
                </div>
                
                <div style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'flex', alignItems: 'center', gap: 6 }}>
                  <Monitor size={13} /> 
                  <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {preset.wallpaper_path ? preset.wallpaper_path.split('/').pop() : "No wallpaper"}
                  </span>
                </div>

                <div style={{ display: 'flex', gap: 6, marginTop: 'auto' }}>
                  <button className="btn btn-primary" style={{ flex: 1, padding: '8px 12px', fontSize: 13 }} onClick={() => onApplyPreset(preset.wallpaper_path, preset.scheme_type, preset.scheme_data)}>
                    <Check size={15} /> Apply
                  </button>
                  <button 
                    className="btn btn-secondary" 
                    style={{ padding: '8px 10px', fontSize: 12 }} 
                    onClick={() => handleExportPreset(preset)}
                    title="Share this preset"
                  >
                    <Share2 size={15} />
                  </button>
                  <button className="btn btn-secondary" style={{ padding: '8px 10px', background: 'rgba(255,100,100,0.1)', color: '#ff6b6b' }} onClick={() => handleDeletePreset(preset.name)}>
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
          <div className="modal-content" style={{ width: 400 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
              <h3>Save Preset</h3>
              <button className="btn" style={{ padding: 4, background: 'transparent' }} onClick={() => setShowSaveModal(false)}>✕</button>
            </div>
            
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <label style={{ fontSize: 14, color: 'var(--text-secondary)' }}>Preset Name</label>
              <input 
                type="text" 
                value={newPresetName}
                onChange={(e) => setNewPresetName(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleSavePreset()}
                placeholder="e.g. Cyberpunk Red"
                style={{ 
                  padding: '12px', 
                  borderRadius: 8, 
                  border: '1px solid var(--border)', 
                  background: 'var(--surface-hover)', 
                  color: 'var(--text-primary)',
                  fontSize: 16
                }}
              />

              <div style={{ 
                padding: 12, 
                borderRadius: 8, 
                background: 'var(--surface-hover)', 
                fontSize: 12, 
                color: 'var(--text-secondary)',
                display: 'flex',
                flexDirection: 'column',
                gap: 4
              }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  <Package size={13} />
                  <span>Preset will include:</span>
                </div>
                <div style={{ paddingLeft: 20 }}>
                  • Color scheme ({currentSchemeType})<br/>
                  • Wallpaper: {currentWallpaper?.split('/').pop() || 'none'}<br/>
                  • All color data for templates
                </div>
              </div>
              
              <div style={{ marginTop: 8, display: 'flex', gap: 12 }}>
                <button className="btn btn-secondary" style={{ flex: 1 }} onClick={() => setShowSaveModal(false)}>Cancel</button>
        {/* Temporarily reverting the debug changes in Presets.tsx */}
        <button className="btn btn-primary" style={{ flex: 1 }} disabled={!newPresetName.trim() || isSaving} onClick={handleSavePreset}>
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
