import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Monitor, Play, Square, RefreshCw, Palette, Eye, EyeOff, CheckCircle2, AlertCircle, Loader2 } from "lucide-react";

interface KdeIntegrationProps {
  schemeData: any;
  wallpaperPath: string | null;
  onGenerateFromWallpaper: (path: string) => Promise<void>;
}

export default function KdeIntegration({ schemeData, onGenerateFromWallpaper }: KdeIntegrationProps) {
  const [serviceEnabled, setServiceEnabled] = useState(false);
  const [currentKdeWallpaper, setCurrentKdeWallpaper] = useState<string | null>(null);
  const [lastAppliedWallpaper, setLastAppliedWallpaper] = useState<string | null>(null);
  const [isApplyingScheme, setIsApplyingScheme] = useState(false);
  const [schemeApplied, setSchemeApplied] = useState(false);
  const [pollInterval, setPollInterval] = useState(5);
  const [statusMessage, setStatusMessage] = useState("");
  const intervalRef = useRef<any>(null);

  useEffect(() => {
    loadServiceStatus();
    fetchCurrentWallpaper();
  }, []);

  // Wallpaper monitor polling
  useEffect(() => {
    if (serviceEnabled) {
      startPolling();
    } else {
      stopPolling();
    }
    return () => stopPolling();
  }, [serviceEnabled, pollInterval]);

  const loadServiceStatus = async () => {
    try {
      const status: any = await invoke("get_kde_service_status");
      setServiceEnabled(status.enabled);
      setLastAppliedWallpaper(status.current_wallpaper);
    } catch (e) {
      console.error("Failed to load service status:", e);
    }
  };

  const fetchCurrentWallpaper = async () => {
    try {
      const wp: string | null = await invoke("get_kde_current_wallpaper");
      setCurrentKdeWallpaper(wp);
    } catch (e) {
      console.error("Failed to get current wallpaper:", e);
    }
  };

  const startPolling = () => {
    stopPolling();
    setStatusMessage("Monitoring wallpaper changes...");
    intervalRef.current = setInterval(async () => {
      try {
        const wp: string | null = await invoke("get_kde_current_wallpaper");
        setCurrentKdeWallpaper(wp);
        
        if (wp && wp !== lastAppliedWallpaper) {
          setStatusMessage(`Wallpaper changed! Generating theme from: ${wp.split('/').pop()}`);
          setLastAppliedWallpaper(wp);
          
          // Auto-generate and apply
          await onGenerateFromWallpaper(wp);
          
          // Small delay for state to propagate, then apply KDE scheme
          setTimeout(async () => {
            try {
              // We need to re-read the scheme data after generation
              // The parent component updates schemeData via onGenerateFromWallpaper
              // but we need to wait for it
              await invoke("apply_kde_colorscheme", { context: schemeData });
              await invoke("apply_theme", { context: schemeData });
              setStatusMessage(`Theme applied automatically from: ${wp.split('/').pop()}`);
            } catch (e) {
              setStatusMessage(`Auto-apply failed: ${e}`);
            }
          }, 1500);
          
          // Save status
          await invoke("set_kde_service_status", { 
            status: { enabled: true, current_wallpaper: wp } 
          });
        }
      } catch (e) {
        // Silently fail on polling errors
      }
    }, pollInterval * 1000);
  };

  const stopPolling = () => {
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
  };

  const toggleService = async () => {
    const newState = !serviceEnabled;
    setServiceEnabled(newState);
    
    try {
      await invoke("set_kde_service_status", { 
        status: { enabled: newState, current_wallpaper: lastAppliedWallpaper } 
      });
      
      if (newState) {
        setStatusMessage("Service started. Monitoring wallpaper changes...");
      } else {
        setStatusMessage("Service stopped.");
        stopPolling();
      }
    } catch (e) {
      alert("Failed to update service: " + e);
    }
  };

  const applyKdeScheme = async () => {
    if (!schemeData) {
      alert("Generate a color palette first from the Colors or Source tab.");
      return;
    }
    
    setIsApplyingScheme(true);
    setSchemeApplied(false);
    try {
      await invoke("apply_kde_colorscheme", { context: schemeData });
      setSchemeApplied(true);
      setStatusMessage("KDE color scheme applied successfully!");
      setTimeout(() => setSchemeApplied(false), 3000);
    } catch (e) {
      alert("Failed to apply KDE color scheme: " + e);
    } finally {
      setIsApplyingScheme(false);
    }
  };

  const generateAndApply = async () => {
    if (!currentKdeWallpaper) {
      alert("No wallpaper detected on KDE desktop.");
      return;
    }
    setIsApplyingScheme(true);
    try {
      await onGenerateFromWallpaper(currentKdeWallpaper);
      setStatusMessage("Colors generated. Applying KDE scheme...");
      
      // Wait for state to propagate
      setTimeout(async () => {
        try {
          await invoke("apply_kde_colorscheme", { context: schemeData });
          await invoke("apply_theme", { context: schemeData });
          setLastAppliedWallpaper(currentKdeWallpaper);
          setStatusMessage("Theme generated and applied from current wallpaper!");
          setSchemeApplied(true);
          setTimeout(() => setSchemeApplied(false), 3000);
        } catch (e) {
          setStatusMessage("Failed to apply: " + e);
        } finally {
          setIsApplyingScheme(false);
        }
      }, 1500);
    } catch (e) {
      alert("Failed to generate colors: " + e);
      setIsApplyingScheme(false);
    }
  };

  // Extract color preview — raw matugen data uses "color" key with #RRGGBBAA format
  const getPreviewColor = (name: string, fallbackName?: string) => {
    const extractHex = (c: any) => {
      if (!c) return null;
      // Try default → dark → light variant, and "color" → "hex" key
      for (const variant of ['default', 'dark', 'light']) {
        const v = c[variant];
        if (!v) continue;
        const raw = v.color || v.hex;
        if (raw) {
          // Strip alpha from #RRGGBBAA → #RRGGBB
          return raw.length === 9 && raw.startsWith('#') ? '#' + raw.slice(1, 7) : raw;
        }
      }
      return null;
    };
    return extractHex(schemeData?.colors?.[name]) 
      || (fallbackName && extractHex(schemeData?.colors?.[fallbackName])) 
      || "#333";
  };

  return (
    <div className="tab-content" style={{ display: 'flex', flexDirection: 'column', gap: 24, height: '100%' }}>
      
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h2>KDE Plasma Integration</h2>
          <p style={{ color: 'var(--text-secondary)' }}>System-wide color scheme and wallpaper monitoring</p>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 20, flex: 1, overflowY: 'auto' }}>
        
        {/* Left Column - Wallpaper Monitor */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
          
          {/* Current Wallpaper Card */}
          <div className="card" style={{ padding: 20 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 16 }}>
              <Monitor size={20} color="var(--accent)" />
              <h3 style={{ margin: 0 }}>Current KDE Wallpaper</h3>
              <button 
                className="btn btn-secondary btn-compact"
                style={{ marginLeft: 'auto' }}
                onClick={fetchCurrentWallpaper}
              >
                <RefreshCw size={14} /> Refresh
              </button>
            </div>
            
            {currentKdeWallpaper ? (
              <div style={{ 
                background: 'var(--surface-hover)', 
                padding: 12, 
                borderRadius: 12, 
                fontSize: 13, 
                wordBreak: 'break-all',
                color: 'var(--text-primary)',
                border: '1px solid var(--border)'
              }}>
                📎 {currentKdeWallpaper}
              </div>
            ) : (
              <div style={{ 
                padding: 12, 
                borderRadius: 12, 
                background: 'rgba(255,180,0,0.1)', 
                color: '#ffb400',
                fontSize: 13 
              }}>
                <AlertCircle size={14} style={{ verticalAlign: 'middle', marginRight: 6 }} />
                Could not detect current wallpaper
              </div>
            )}

            <button 
              className="btn btn-primary" 
              style={{ width: '100%', marginTop: 12 }}
              disabled={!currentKdeWallpaper || isApplyingScheme}
              onClick={generateAndApply}
            >
              {isApplyingScheme ? (
                <><Loader2 size={16} className="spinning" /> Generating...</>
              ) : (
                <><Palette size={16} /> Generate from Current Wallpaper</>
              )}
            </button>
          </div>

          {/* Wallpaper Watch Service */}
          <div className="card" style={{ padding: 20 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 16 }}>
              {serviceEnabled ? <Eye size={20} color="#4ade80" /> : <EyeOff size={20} color="var(--text-secondary)" />}
              <h3 style={{ margin: 0 }}>Wallpaper Watch Service</h3>
            </div>
            
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 16 }}>
              When enabled, Matugen Studio will monitor your KDE wallpaper. Whenever you change it 
              (via right-click → Set as Wallpaper, or any other method), the app automatically 
              generates a new color scheme and applies it system-wide.
            </p>

            <div style={{ display: 'flex', gap: 12, alignItems: 'center', marginBottom: 16 }}>
              <button 
                className={`btn ${serviceEnabled ? 'btn-secondary' : 'btn-primary'}`}
                onClick={toggleService}
                style={{ flex: 1 }}
              >
                {serviceEnabled ? (
                  <><Square size={16} /> Stop Service</>
                ) : (
                  <><Play size={16} /> Start Service</>
                )}
              </button>
            </div>

            <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
              <label style={{ fontSize: 13, color: 'var(--text-secondary)', whiteSpace: 'nowrap' }}>
                Poll interval:
              </label>
              <select 
                value={pollInterval} 
                onChange={(e) => setPollInterval(parseInt(e.target.value))}
                style={{ minWidth: 132 }}
              >
                <option value={2}>2 seconds</option>
                <option value={5}>5 seconds</option>
                <option value={10}>10 seconds</option>
                <option value={30}>30 seconds</option>
                <option value={60}>1 minute</option>
              </select>
            </div>

            {/* Status */}
            {statusMessage && (
              <div style={{ 
                marginTop: 12, 
                padding: 10, 
                borderRadius: 8, 
                background: serviceEnabled ? 'rgba(74, 222, 128, 0.1)' : 'var(--surface-hover)',
                color: serviceEnabled ? '#4ade80' : 'var(--text-secondary)',
                fontSize: 12,
                display: 'flex',
                alignItems: 'center',
                gap: 8
              }}>
                {serviceEnabled && <span className="pulse-dot" />}
                {statusMessage}
              </div>
            )}
          </div>
        </div>

        {/* Right Column - KDE Color Scheme */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>

          {/* Apply KDE Scheme Card */}
          <div className="card" style={{ padding: 20 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 16 }}>
              <Palette size={20} color="var(--accent)" />
              <h3 style={{ margin: 0 }}>KDE Color Scheme</h3>
            </div>
            
            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 16 }}>
              Generate a <code>MatugenStudio.colors</code> file and apply it as your system-wide KDE Plasma color scheme.
              This affects window decorations, buttons, menus, system tray, and all Qt/KDE apps.
            </p>

            <button 
              className="btn btn-primary" 
              style={{ width: '100%' }}
              disabled={!schemeData || isApplyingScheme}
              onClick={applyKdeScheme}
            >
              {isApplyingScheme ? (
                <><Loader2 size={16} className="spinning" /> Applying...</>
              ) : schemeApplied ? (
                <><CheckCircle2 size={16} /> Applied!</>
              ) : (
                <><Palette size={16} /> Apply KDE Color Scheme</>
              )}
            </button>

            {!schemeData && (
              <p style={{ fontSize: 12, color: 'var(--text-secondary)', marginTop: 8, textAlign: 'center' }}>
                Generate colors first from the Source or Colors tab.
              </p>
            )}
          </div>

          {/* Color Preview */}
          {schemeData && (
            <div className="card" style={{ padding: 20 }}>
              <h3 style={{ marginTop: 0, marginBottom: 16 }}>Scheme Preview</h3>
              
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {[
                  { label: "Window Background", color: getPreviewColor("surface_container_low", "surface") },
                  { label: "View Background", color: getPreviewColor("surface") },
                  { label: "Button", color: getPreviewColor("surface_container", "surface") },
                  { label: "Selection", color: getPreviewColor("primary") },
                  { label: "Foreground", color: getPreviewColor("on_surface") },
                  { label: "Link / Active", color: getPreviewColor("primary") },
                  { label: "Negative", color: getPreviewColor("error") },
                  { label: "Neutral", color: getPreviewColor("tertiary") },
                  { label: "Visited", color: getPreviewColor("secondary") },
                ].map((item, idx) => (
                  <div key={idx} style={{ 
                    display: 'flex', 
                    alignItems: 'center', 
                    gap: 12, 
                    padding: '6px 8px',
                    borderRadius: 8,
                    background: 'var(--surface-hover)'
                  }}>
                    <div style={{ 
                      width: 24, 
                      height: 24, 
                      borderRadius: 6, 
                      background: item.color, 
                      border: '1px solid var(--border)',
                      flexShrink: 0
                    }} />
                    <span style={{ fontSize: 13, color: 'var(--text-secondary)' }}>{item.label}</span>
                    <span style={{ fontSize: 11, color: 'var(--text-secondary)', marginLeft: 'auto', fontFamily: 'monospace' }}>
                      {item.color.toUpperCase()}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
