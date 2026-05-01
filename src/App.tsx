import { useState } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Image, Palette, LayoutTemplate, Monitor, Settings, Download, Copy, Edit2, Check, X } from "lucide-react";
import Wheel from '@uiw/react-color-wheel';
import ShadeSlider from '@uiw/react-color-shade-slider';
import { hsvaToHex, hexToHsva, hexToRgba } from '@uiw/color-convert';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import Templates from "./pages/Templates";
import Source from "./pages/Source";
import Presets from "./pages/Presets";
import KdeIntegration from "./pages/KdeIntegration";
import "./App.css";

function App() {
  const [activeTab, setActiveTab] = useState("colors");
  const [wallpaperPath, setWallpaperPath] = useState<string | null>(null);
  const [schemeData, setSchemeData] = useState<any>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [schemeType, setSchemeType] = useState("Content");
  const [selectedColor, setSelectedColor] = useState<{name: string, path: string, hex: string, originalHex: string} | null>(null);
  const [showPicker, setShowPicker] = useState(false);
  const [pickerHsva, setPickerHsva] = useState({ h: 0, s: 0, v: 0, a: 1 });
  const [copied, setCopied] = useState(false);

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
      const rawData = await invoke("generate_scheme_from_image", { imagePath: path, schemeType });
      const data = fixMatugenColors(rawData);
      setSchemeData(data);
      setErrorMsg(null);
      // Wait for React to apply state, then apply theme + KDE scheme
      setTimeout(async () => {
        try {
          await invoke("apply_theme", { context: data });
          // Also apply KDE color scheme automatically (runs in background, won't block)
          invoke("apply_kde_colorscheme", { context: data }).catch(e => 
            console.error("KDE scheme apply failed:", e)
          );
          alert("Theme generated and applied globally!");
        } catch (err) {
          alert("Generated colors but failed to apply theme globally: " + err);
        }
      }, 500);
    } catch (err: any) {
      setErrorMsg(err.toString());
      alert("Failed to generate colors: " + err);
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
      setSelectedColor(prev => prev ? { ...prev, hex } : null);
    }
  };

  const handleApplyPreset = async (wallpaper: string | null, scheme_type: string, scheme_data: any) => {
    setWallpaperPath(wallpaper);
    setSchemeType(scheme_type);
    setSchemeData(scheme_data);
    
    try {
      if (wallpaper) {
        await invoke("apply_wallpaper", { imagePath: wallpaper, screenIndex: -1 });
      }
      await invoke("apply_theme", { context: scheme_data });
      invoke("apply_kde_colorscheme", { context: scheme_data }).catch(e => 
        console.error("KDE scheme apply failed:", e)
      );
      alert("Preset applied globally!");
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
          setSelectedColor({name, path, hex, originalHex: hex}); 
          setPickerHsva(hexToHsva(hex));
          setShowPicker(false); 
        }}
      >
        <span style={{opacity: 0.7}}>{name}</span>
        <span style={{fontWeight: 'bold'}}>{hex.toUpperCase()}</span>
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
              <h3>{selectedColor.name.replace('-', ' ')}</h3>
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
                    setSelectedColor(null); 
                    setShowPicker(false); 
                    if (schemeData) {
                      try {
                        await invoke("apply_theme", { context: schemeData });
                        // alert("Theme applied!"); // don't alert to keep it seamless
                      } catch (e) {
                        console.error(e);
                      }
                    }
                  }}
                >
                  <Check size={16} /> Apply Changes to System
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
            await invoke("apply_theme", { context: schemeData });
            invoke("apply_kde_colorscheme", { context: schemeData }).catch(e => 
              console.error("KDE scheme apply failed:", e)
            );
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
            onApplyAndGenerate={handleApplyAndGenerate} 
            onSelectForColors={handleSelectForColors} 
          />
        )}

        {activeTab === 'presets' && (
          <Presets 
            currentWallpaper={wallpaperPath}
            currentSchemeType={schemeType}
            currentSchemeData={schemeData}
            onApplyPreset={handleApplyPreset}
          />
        )}

        {activeTab === 'colors' && (
          <>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <h2>Matugen Studio</h2>
              <div className="controls-row">
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

            <div className="dashboard-grid">
            {/* Left Column */}
            <div className="card">
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
            <div className="card">
              <h3>Material You Palette</h3>
              
              {schemeData ? (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', overflowY: 'auto', paddingRight: '8px' }}>
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
            </div>
          </>
        )}

        {activeTab === 'templates' && (
          <Templates schemeData={schemeData} />
        )}

        {activeTab === 'desktop' && (
          <KdeIntegration 
            schemeData={schemeData}
            wallpaperPath={wallpaperPath}
            onGenerateFromWallpaper={async (path: string) => {
              setWallpaperPath(path);
              try {
                setIsLoading(true);
                const rawData = await invoke("generate_scheme_from_image", { imagePath: path, schemeType });
                const data = fixMatugenColors(rawData);
                setSchemeData(data);
                setErrorMsg(null);
              } catch (err: any) {
                setErrorMsg(err.toString());
              } finally {
                setIsLoading(false);
              }
            }}
          />
        )}
      </main>
    </div>
  );
}

export default App;
