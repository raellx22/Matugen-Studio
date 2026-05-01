import React, { useState, useEffect, useMemo } from 'react';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { FolderOpen, Star, Shuffle, Monitor, Palette } from 'lucide-react';

interface SourceProps {
  onApplyAndGenerate: (path: string) => Promise<void>;
  onSelectForColors: (path: string) => void;
}

const Thumbnail = ({ path }: { path: string }) => {
  const [thumbPath, setThumbPath] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;
    invoke<string>('generate_thumbnail', { imagePath: path })
      .then(res => {
         if (mounted) setThumbPath(res);
      })
      .catch(e => console.error(e));
    return () => { mounted = false; };
  }, [path]);

  if (!thumbPath) {
    return (
      <div style={{ width: '100%', height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#111' }}>
        <span style={{ fontSize: 12, color: '#666' }}>Loading thumbnail...</span>
      </div>
    );
  }

  return (
    <img 
      src={convertFileSrc(thumbPath)} 
      alt="Wallpaper" 
      loading="lazy" 
      decoding="async" 
      onError={(e) => { e.currentTarget.src = convertFileSrc(path); }}
      style={{ width: '100%', height: '100%', objectFit: 'cover', animation: 'fadeIn 0.3s ease' }} 
    />
  );
};

export default function Source({ onApplyAndGenerate, onSelectForColors }: SourceProps) {
  // Multi-Monitor State
  const [monitorCount, setMonitorCount] = useState(1);
  const [multiMonitorEnabled, setMultiMonitorEnabled] = useState(localStorage.getItem('multiMonitorEnabled') === 'true');
  const [activeMonitor, setActiveMonitor] = useState(-1); // -1 means Global, 0+ means specific screen
  const [mainColorMonitor, setMainColorMonitor] = useState(parseInt(localStorage.getItem('mainColorMonitor') || '0'));

  // Wallpaper State
  const [wallpapers, setWallpapers] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [favorites, setFavorites] = useState<string[]>(JSON.parse(localStorage.getItem('favorites') || '[]'));

  // Visible items count for lazy rendering
  const [visibleCount, setVisibleCount] = useState(40);

  const getFolderKey = (monitorId: number) => `wallpaperFolder_${monitorId}`;
  
  const getCurrentFolder = (monitorId: number) => {
    let folder = localStorage.getItem(getFolderKey(monitorId));
    // Fallback to global folder if specific monitor folder is not set
    if (!folder && monitorId !== -1) {
      folder = localStorage.getItem(getFolderKey(-1));
    }
    return folder;
  };
  
  const [currentFolder, setCurrentFolder] = useState<string | null>(getCurrentFolder(-1));

  useEffect(() => {
    invoke<number>('get_monitor_count').then(count => {
      setMonitorCount(Math.max(1, count));
    }).catch(e => console.error(e));
  }, []);

  useEffect(() => {
    const folder = getCurrentFolder(activeMonitor);
    setCurrentFolder(folder);
    setVisibleCount(40);
    if (folder) {
      loadWallpapers(folder);
    } else {
      setWallpapers([]);
    }
  }, [activeMonitor]);

  const loadWallpapers = async (folder: string) => {
    setIsLoading(true);
    try {
      const result: string[] = await invoke('list_wallpapers', { folderPath: folder });
      setWallpapers(result);
    } catch (e) {
      console.error("Failed to load wallpapers", e);
    } finally {
      setIsLoading(false);
    }
  };

  const selectFolder = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected && typeof selected === 'string') {
      setCurrentFolder(selected);
      localStorage.setItem(getFolderKey(activeMonitor), selected);
      loadWallpapers(selected);
    }
  };

  const toggleFavorite = (path: string) => {
    let newFavs = [...favorites];
    if (newFavs.includes(path)) {
      newFavs = newFavs.filter(f => f !== path);
    } else {
      newFavs.push(path);
    }
    setFavorites(newFavs);
    localStorage.setItem('favorites', JSON.stringify(newFavs));
  };

  const applyWallpaperOnly = async (path: string) => {
    try {
      await invoke('apply_wallpaper', { imagePath: path, screenIndex: multiMonitorEnabled ? activeMonitor : -1 });
      alert("Wallpaper applied successfully in KDE!");
    } catch (e) {
      alert("Failed to apply wallpaper: " + e);
    }
  };

  const handleApplyAndGenerate = async (path: string) => {
    try {
      await invoke('apply_wallpaper', { imagePath: path, screenIndex: multiMonitorEnabled ? activeMonitor : -1 });
      
      if (!multiMonitorEnabled || activeMonitor === -1 || activeMonitor === mainColorMonitor) {
        await onApplyAndGenerate(path);
      } else {
        alert("Wallpaper applied to monitor! (Colors were not generated because this is not set as your Main Color Monitor).");
      }
    } catch (e) {
      alert("Failed to apply and generate: " + e);
    }
  };

  const pickRandomFavorite = async () => {
    if (favorites.length === 0) {
      alert("No favorites added! Please click the star on your wallpapers first.");
      return;
    }
    const randomPath = favorites[Math.floor(Math.random() * favorites.length)];
    await handleApplyAndGenerate(randomPath);
  };

  const sortedWallpapers = useMemo(() => {
    return [...wallpapers].sort((a, b) => {
      const aFav = favorites.includes(a);
      const bFav = favorites.includes(b);
      if (aFav && !bFav) return -1;
      if (!aFav && bFav) return 1;
      if (aFav && bFav) {
        return favorites.indexOf(a) - favorites.indexOf(b);
      }
      return a.localeCompare(b);
    });
  }, [wallpapers, favorites]);

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const bottom = e.currentTarget.scrollHeight - e.currentTarget.scrollTop <= e.currentTarget.clientHeight + 400;
    if (bottom && visibleCount < sortedWallpapers.length) {
      setVisibleCount(prev => prev + 20);
    }
  };

  return (
    <div className="source-page" style={{ padding: 24, display: 'flex', flexDirection: 'column', gap: 20, height: '100%', width: '100%' }}>
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h2 style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            Wallpaper Source
          </h2>
          <p style={{ color: 'var(--text-secondary)', fontSize: 13, wordBreak: 'break-all' }}>
            {currentFolder ? currentFolder : "Select a folder to load your wallpapers"}
          </p>
        </div>
        <div style={{ display: 'flex', gap: 12 }}>
          <button 
            className={`btn ${multiMonitorEnabled ? 'btn-primary' : 'btn-secondary'}`} 
            onClick={() => {
              const newVal = !multiMonitorEnabled;
              setMultiMonitorEnabled(newVal);
              localStorage.setItem('multiMonitorEnabled', newVal.toString());
              if (!newVal) setActiveMonitor(-1);
            }}
            title="Enable independent wallpapers per monitor"
          >
            <Monitor size={18} />
            {multiMonitorEnabled ? "Multi-Monitor ON" : "Multi-Monitor OFF"}
          </button>
        </div>
      </div>

      {/* Multi-Monitor Controls */}
      {multiMonitorEnabled && (
        <div style={{ background: 'var(--surface)', padding: 16, borderRadius: 16, border: '1px solid var(--border)', display: 'flex', gap: 24, alignItems: 'center' }}>
          <div style={{ display: 'flex', gap: 8, flex: 1, flexWrap: 'wrap' }}>
            <button 
              className={`btn ${activeMonitor === -1 ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setActiveMonitor(-1)}
            >
              Global (All)
            </button>
            {Array.from({ length: monitorCount }).map((_, idx) => (
              <button 
                key={idx}
                className={`btn ${activeMonitor === idx ? 'btn-primary' : 'btn-secondary'}`}
                onClick={() => setActiveMonitor(idx)}
              >
                Monitor {idx + 1}
              </button>
            ))}
          </div>
          
          <div style={{ display: 'flex', alignItems: 'center', gap: 12, borderLeft: '1px solid var(--border)', paddingLeft: 24 }}>
            <span style={{ fontSize: 13, color: 'var(--text-secondary)', whiteSpace: 'nowrap' }}>Main Color Monitor:</span>
            <select 
              value={mainColorMonitor} 
              onChange={(e) => {
                const val = parseInt(e.target.value);
                setMainColorMonitor(val);
                localStorage.setItem('mainColorMonitor', val.toString());
              }}
              style={{ padding: '8px 12px', borderRadius: 8, background: 'var(--surface-hover)', border: '1px solid var(--border)', color: 'var(--text-primary)' }}
            >
              {Array.from({ length: monitorCount }).map((_, idx) => (
                <option key={idx} value={idx}>Monitor {idx + 1}</option>
              ))}
            </select>
          </div>
        </div>
      )}

      {/* Actions Row */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ display: 'flex', gap: 12 }}>
          {favorites.length > 0 && (
            <button className="btn btn-secondary" onClick={pickRandomFavorite}>
              <Shuffle size={18} />
              Shuffle Favorites
            </button>
          )}
        </div>
        <button className="btn btn-secondary" onClick={selectFolder}>
          <FolderOpen size={18} />
          {currentFolder ? "Change Folder" : "Select Folder"}
        </button>
      </div>

      {/* Gallery */}
      {!currentFolder ? (
        <div className="drop-zone" onClick={selectFolder} style={{ cursor: 'pointer', flex: 1, display: 'flex', justifyContent: 'center', alignItems: 'center', borderRadius: 16, border: '2px dashed var(--border)', background: 'var(--surface)' }}>
          <div style={{ textAlign: 'center', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 16 }}>
            <FolderOpen size={48} color="var(--accent)" />
            <p style={{ margin: 0, fontSize: 18, color: 'var(--text-secondary)' }}>Select a folder for {activeMonitor === -1 ? 'Global' : `Monitor ${activeMonitor + 1}`}</p>
            <p style={{ margin: 0, fontSize: 14, color: 'var(--text-secondary)', opacity: 0.7 }}>We will load a beautiful gallery for you</p>
          </div>
        </div>
      ) : isLoading ? (
        <div style={{ flex: 1, display: 'flex', justifyContent: 'center', alignItems: 'center' }}>
          <p>Loading wallpapers...</p>
        </div>
      ) : sortedWallpapers.length === 0 ? (
        <p>No wallpapers found in this directory. Only JPG, PNG and WEBP are supported.</p>
      ) : (
        <div 
          onScroll={handleScroll}
          style={{ 
            display: 'grid', 
            gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', 
            gap: 16, 
            overflowY: 'auto', 
            paddingRight: 8,
            paddingBottom: 24,
            flex: 1 
          }}
        >
          {sortedWallpapers.slice(0, visibleCount).map((path, idx) => {
            const isFav = favorites.includes(path);
            return (
              <div key={idx} className="template-card" style={{
                background: 'var(--surface)',
                borderRadius: 16,
                border: '1px solid var(--border)',
                display: 'flex',
                flexDirection: 'column',
                overflow: 'hidden',
                position: 'relative',
                height: 260
              }}>
                <div style={{ position: 'absolute', top: 8, right: 8, zIndex: 10, display: 'flex', gap: 8 }}>
                  <button 
                    className="btn" 
                    style={{ 
                      padding: 8, 
                      background: 'rgba(0,0,0,0.5)', 
                      backdropFilter: 'blur(4px)',
                      border: '1px solid rgba(255,255,255,0.1)',
                      color: '#fff'
                    }}
                    onClick={() => onSelectForColors(path)}
                    title="Load colors into Editor"
                  >
                    <Palette size={18} />
                  </button>
                  <button 
                    className="btn" 
                    style={{ 
                      padding: 8, 
                      background: 'rgba(0,0,0,0.5)', 
                      backdropFilter: 'blur(4px)',
                      border: '1px solid rgba(255,255,255,0.1)',
                      color: isFav ? '#ffd700' : '#fff'
                    }}
                    onClick={() => toggleFavorite(path)}
                    title="Toggle Favorite"
                  >
                    <Star size={18} fill={isFav ? '#ffd700' : 'none'} />
                  </button>
                </div>
                <div style={{ height: 160, width: '100%', overflow: 'hidden', background: '#111' }}>
                  <Thumbnail path={path} />
                </div>
                <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 8, flex: 1, justifyContent: 'center' }}>
                  <button className="btn btn-secondary" style={{ padding: '8px 12px', fontSize: 13, width: '100%' }} onClick={() => applyWallpaperOnly(path)} title="Sets the wallpaper in KDE only">
                    Apply Only (KDE)
                  </button>
                  <button className="btn btn-primary" style={{ padding: '8px 12px', fontSize: 13, width: '100%' }} onClick={() => handleApplyAndGenerate(path)} title="Sets wallpaper, generates colors, and updates all templates">
                    Apply & Generate
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}
      
      <style>
        {`
          @keyframes fadeIn {
            from { opacity: 0; }
            to { opacity: 1; }
          }
        `}
      </style>
    </div>
  );
}
