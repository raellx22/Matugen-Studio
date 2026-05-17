import { useState, useEffect, useMemo, useRef, type CSSProperties, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { ChevronLeft, ChevronRight, FolderOpen, Monitor, Palette, Shuffle, Star } from 'lucide-react';
import { getWallpaperPage } from '../utils/wallpaperPagination';

interface SourceProps {
  itemsPerPage: number;
  onApplyAndGenerate: (path: string) => Promise<void>;
  onSelectForColors: (path: string) => void;
}

interface ThumbnailProps {
  thumbnailPath: string | null;
  hasError: boolean;
}

interface ThumbnailResult {
  imagePath: string;
  thumbnailPath: string | null;
  error: string | null;
}

interface CompactPaginationButtonProps {
  label: string;
  title: string;
  disabled: boolean;
  onClick: () => void;
  children: ReactNode;
}

const compactPaginationButtonStyle = (disabled: boolean): CSSProperties => ({
  height: 36,
  minWidth: 42,
  padding: '8px 12px',
  borderRadius: 999,
  border: '1px solid var(--border)',
  background: disabled ? 'rgba(255,255,255,0.03)' : 'var(--surface)',
  color: disabled ? 'rgba(255,255,255,0.32)' : 'var(--text-primary)',
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  gap: 6,
  fontSize: 13,
  lineHeight: 1,
  cursor: disabled ? 'not-allowed' : 'pointer',
  opacity: disabled ? 0.6 : 1,
});

const CompactPaginationButton = ({ label, title, disabled, onClick, children }: CompactPaginationButtonProps) => (
  <button
    type="button"
    aria-label={label}
    title={title}
    disabled={disabled}
    onClick={onClick}
    style={compactPaginationButtonStyle(disabled)}
  >
    {children}
  </button>
);

const Thumbnail = ({ thumbnailPath, hasError }: ThumbnailProps) => {
  if (!thumbnailPath && !hasError) {
    return (
      <div style={{ width: '100%', height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#111' }}>
        <span style={{ fontSize: 12, color: '#666' }}>Loading thumbnail...</span>
      </div>
    );
  }

  if (hasError) {
    return (
      <div style={{ width: '100%', height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center', background: '#111', padding: 12, textAlign: 'center' }}>
        <span style={{ fontSize: 12, color: '#777' }}>Thumbnail unavailable</span>
      </div>
    );
  }

  if (!thumbnailPath) {
    return null;
  }

  return (
    <img
      src={convertFileSrc(thumbnailPath)}
      alt="Wallpaper"
      loading="lazy"
      decoding="async"
      style={{ width: '100%', height: '100%', objectFit: 'cover' }}
    />
  );
};

export default function Source({ itemsPerPage, onApplyAndGenerate, onSelectForColors }: SourceProps) {
  const { t } = useTranslation();
  const [monitorCount, setMonitorCount] = useState(1);
  const [multiMonitorEnabled, setMultiMonitorEnabled] = useState(localStorage.getItem('multiMonitorEnabled') === 'true');
  const [activeMonitor, setActiveMonitor] = useState(-1);
  const [mainColorMonitor, setMainColorMonitor] = useState(parseInt(localStorage.getItem('mainColorMonitor') || '0'));

  const [wallpapers, setWallpapers] = useState<string[]>([]);
  const [isScanningSource, setIsScanningSource] = useState(false);
  const [isLoadingPage, setIsLoadingPage] = useState(false);
  const [favorites, setFavorites] = useState<string[]>(JSON.parse(localStorage.getItem('favorites') || '[]'));
  const [currentPage, setCurrentPage] = useState(1);
  const [thumbnailPaths, setThumbnailPaths] = useState<Record<string, string>>({});
  const [thumbnailErrors, setThumbnailErrors] = useState<Record<string, boolean>>({});

  const scanGenerationRef = useRef(0);
  const pageGenerationRef = useRef(0);
  const thumbnailCacheRef = useRef<Map<string, string>>(new Map());

  const getFolderKey = (monitorId: number) => `wallpaperFolder_${monitorId}`;

  const getCurrentFolder = (monitorId: number) => {
    let folder = localStorage.getItem(getFolderKey(monitorId));
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

  const clearPageCache = () => {
    thumbnailCacheRef.current.clear();
    setThumbnailPaths({});
    setThumbnailErrors({});
  };

  const loadWallpapers = async (folder: string) => {
    const generation = ++scanGenerationRef.current;
    ++pageGenerationRef.current;
    setIsScanningSource(true);
    setIsLoadingPage(false);
    setCurrentPage(1);
    setWallpapers([]);
    clearPageCache();

    try {
      const result: string[] = await invoke('list_wallpapers', { folderPath: folder });
      if (scanGenerationRef.current === generation) {
        setWallpapers(result);
      }
    } catch (e) {
      console.error("Failed to load wallpapers", e);
    } finally {
      if (scanGenerationRef.current === generation) {
        setIsScanningSource(false);
      }
    }
  };

  useEffect(() => {
    const folder = getCurrentFolder(activeMonitor);
    setCurrentFolder(folder);
    if (folder) {
      loadWallpapers(folder);
    } else {
      ++scanGenerationRef.current;
      ++pageGenerationRef.current;
      setWallpapers([]);
      setCurrentPage(1);
      setIsScanningSource(false);
      setIsLoadingPage(false);
      clearPageCache();
    }
  }, [activeMonitor]);

  useEffect(() => {
    setCurrentPage(1);
  }, [itemsPerPage]);

  const favoriteSet = useMemo(() => new Set(favorites), [favorites]);
  const favoriteOrder = useMemo(
    () => new Map(favorites.map((path, index) => [path, index])),
    [favorites],
  );

  const sortedWallpapers = useMemo(() => {
    return [...wallpapers].sort((a, b) => {
      const aFav = favoriteSet.has(a);
      const bFav = favoriteSet.has(b);
      if (aFav && !bFav) return -1;
      if (!aFav && bFav) return 1;
      if (aFav && bFav) {
        return (favoriteOrder.get(a) ?? 0) - (favoriteOrder.get(b) ?? 0);
      }
      return a.localeCompare(b);
    });
  }, [wallpapers, favoriteSet, favoriteOrder]);

  const page = useMemo(
    () => getWallpaperPage(sortedWallpapers, currentPage, itemsPerPage),
    [sortedWallpapers, currentPage, itemsPerPage],
  );

  useEffect(() => {
    if (currentPage !== page.currentPage) {
      setCurrentPage(page.currentPage);
    }
  }, [currentPage, page.currentPage]);

  const pruneThumbnailCache = (allowedPaths: Set<string>) => {
    for (const path of thumbnailCacheRef.current.keys()) {
      if (!allowedPaths.has(path)) {
        thumbnailCacheRef.current.delete(path);
      }
    }

    setThumbnailPaths(prev => {
      const next: Record<string, string> = {};
      for (const [path, thumbPath] of Object.entries(prev)) {
        if (allowedPaths.has(path)) {
          next[path] = thumbPath;
        }
      }
      return next;
    });

    setThumbnailErrors(prev => {
      const next: Record<string, boolean> = {};
      for (const [path, failed] of Object.entries(prev)) {
        if (allowedPaths.has(path)) {
          next[path] = failed;
        }
      }
      return next;
    });
  };

  const loadThumbnails = async (paths: string[], generation: number) => {
    try {
      const results = await invoke<ThumbnailResult[]>('generate_thumbnails', { imagePaths: paths });
      if (pageGenerationRef.current !== generation) {
        return;
      }

      const nextPaths: Record<string, string> = {};
      const nextErrors: Record<string, boolean> = {};

      for (const result of results) {
        if (result.thumbnailPath) {
          thumbnailCacheRef.current.set(result.imagePath, result.thumbnailPath);
          nextPaths[result.imagePath] = result.thumbnailPath;
        } else {
          if (result.error) {
            console.error("Failed to generate thumbnail", result.error);
          }
          nextErrors[result.imagePath] = true;
        }
      }

      if (Object.keys(nextPaths).length > 0) {
        setThumbnailPaths(prev => ({ ...prev, ...nextPaths }));
      }

      if (Object.keys(nextErrors).length > 0) {
        setThumbnailErrors(prev => ({ ...prev, ...nextErrors }));
      }
    } catch (e) {
      if (pageGenerationRef.current === generation) {
        console.error("Failed to generate thumbnails", e);
        setThumbnailErrors(prev => {
          const next = { ...prev };
          paths.forEach(path => {
            next[path] = true;
          });
          return next;
        });
      }
    } finally {
      if (pageGenerationRef.current === generation) {
        setIsLoadingPage(false);
      }
    }
  };

  useEffect(() => {
    const generation = ++pageGenerationRef.current;
    const currentPageItems = page.items;
    const previousPageItems = getWallpaperPage(sortedWallpapers, page.currentPage - 1, itemsPerPage).items;
    const cachedWindow = new Set([...previousPageItems, ...currentPageItems]);

    pruneThumbnailCache(cachedWindow);

    if (currentPageItems.length === 0) {
      setIsLoadingPage(false);
      return;
    }

    const cachedForPage = currentPageItems.filter(path => thumbnailCacheRef.current.has(path));
    if (cachedForPage.length > 0) {
      setThumbnailPaths(prev => {
        const next = { ...prev };
        cachedForPage.forEach(path => {
          const cached = thumbnailCacheRef.current.get(path);
          if (cached) next[path] = cached;
        });
        return next;
      });
    }

    const missingCurrentPage = currentPageItems.filter(path => !thumbnailCacheRef.current.has(path));
    setIsLoadingPage(missingCurrentPage.length > 0);
    if (missingCurrentPage.length > 0) {
      void loadThumbnails(missingCurrentPage, generation);
    }
  }, [page.currentPage, page.items, sortedWallpapers, itemsPerPage]);

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

  const goToPage = (nextPage: number) => {
    setCurrentPage(Math.min(Math.max(1, nextPage), page.totalPages));
  };

  return (
    <div className="source-page" style={{ padding: 24, display: 'flex', flexDirection: 'column', gap: 20, height: '100%', width: '100%' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h2 style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            {t('source.title')}
          </h2>
          <p style={{ color: 'var(--text-secondary)', fontSize: 13, wordBreak: 'break-all' }}>
            {currentFolder ? currentFolder : t('source.selectFolderSubtitle')}
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

      {multiMonitorEnabled && (
        <div style={{ background: 'var(--surface)', padding: 16, borderRadius: 16, border: '1px solid var(--border)', display: 'flex', gap: 24, alignItems: 'center' }}>
          <div style={{ display: 'flex', gap: 8, flex: 1, flexWrap: 'wrap' }}>
            <button
              className={`btn ${activeMonitor === -1 ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setActiveMonitor(-1)}
            >
              {t('source.globalAll')}
            </button>
            {Array.from({ length: monitorCount }).map((_, idx) => (
              <button
                key={idx}
                className={`btn ${activeMonitor === idx ? 'btn-primary' : 'btn-secondary'}`}
                onClick={() => setActiveMonitor(idx)}
              >
                {t('source.monitorPrefix')} {idx + 1}
              </button>
            ))}
          </div>

          <div style={{ display: 'flex', alignItems: 'center', gap: 12, borderLeft: '1px solid var(--border)', paddingLeft: 24 }}>
            <span style={{ fontSize: 13, color: 'var(--text-secondary)', whiteSpace: 'nowrap' }}>{t('source.mainColorMonitor')}</span>
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
                <option key={idx} value={idx}>{t('source.monitorPrefix')} {idx + 1}</option>
              ))}
            </select>
          </div>
        </div>
      )}

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
        <div style={{ display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}>
          {favorites.length > 0 && (
            <button className="btn btn-secondary" onClick={pickRandomFavorite}>
              <Shuffle size={18} />
              {t('source.shuffleFavorites')}
            </button>
          )}
          {currentFolder && sortedWallpapers.length > 0 && (
            <span style={{ color: 'var(--text-secondary)', fontSize: 13 }}>
              {t('source.wallpapersFound', { count: page.totalItems, perPage: itemsPerPage })}
            </span>
          )}
        </div>
        <button className="btn btn-secondary" onClick={selectFolder}>
          <FolderOpen size={18} />
          {currentFolder ? t('source.changeFolder') : t('source.selectFolder')}
        </button>
      </div>

      {!currentFolder ? (
        <div className="drop-zone" onClick={selectFolder} style={{ cursor: 'pointer', flex: 1, display: 'flex', justifyContent: 'center', alignItems: 'center', borderRadius: 16, border: '2px dashed var(--border)', background: 'var(--surface)' }}>
          <div style={{ textAlign: 'center', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 16 }}>
            <FolderOpen size={48} color="var(--accent)" />
            <p style={{ margin: 0, fontSize: 18, color: 'var(--text-secondary)' }}>{t('source.selectFolderFor', { target: activeMonitor === -1 ? t('source.globalAll') : `${t('source.monitorPrefix')} ${activeMonitor + 1}` })}</p>
            <p style={{ margin: 0, fontSize: 14, color: 'var(--text-secondary)', opacity: 0.7 }}>{t('source.onlyCurrentPageLoaded')}</p>
          </div>
        </div>
      ) : isScanningSource ? (
        <div style={{ flex: 1, display: 'flex', justifyContent: 'center', alignItems: 'center' }}>
          <p>{t('source.scanningSource')}</p>
        </div>
      ) : sortedWallpapers.length === 0 ? (
        <p>{t('source.noWallpapersFound')}</p>
      ) : (
        <>
          {isLoadingPage && (
            <div style={{ color: 'var(--text-secondary)', fontSize: 13 }}>
              {t('source.loadingThumbnails', { page: page.currentPage })}
            </div>
          )}
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))',
              gap: 16,
              overflowY: 'auto',
              paddingRight: 8,
              paddingBottom: 24,
              flex: 1,
            }}
          >
            {page.items.map((path) => {
              const isFav = favoriteSet.has(path);
              return (
                <div key={path} className="template-card" style={{
                  background: 'var(--surface)',
                  borderRadius: 16,
                  border: '1px solid var(--border)',
                  display: 'flex',
                  flexDirection: 'column',
                  overflow: 'hidden',
                  position: 'relative',
                  height: 260,
                }}>
                  <div style={{ position: 'absolute', top: 8, right: 8, zIndex: 10, display: 'flex', gap: 8 }}>
                    <button
                      className="btn btn-ghost icon-btn"
                      style={{
                        padding: 0,
                        background: 'rgba(0,0,0,0.5)',
                        border: '1px solid rgba(255,255,255,0.1)',
                        color: '#fff',
                      }}
                      onClick={() => onSelectForColors(path)}
                      title={t('source.loadColors')}
                    >
                      <Palette size={18} />
                    </button>
                    <button
                      className="btn btn-ghost icon-btn"
                      style={{
                        padding: 0,
                        background: 'rgba(0,0,0,0.5)',
                        border: '1px solid rgba(255,255,255,0.1)',
                        color: isFav ? '#ffd700' : '#fff',
                      }}
                      onClick={() => toggleFavorite(path)}
                      title={t('source.toggleFavorite')}
                    >
                      <Star size={18} fill={isFav ? '#ffd700' : 'none'} />
                    </button>
                  </div>
                  <div style={{ height: 160, width: '100%', overflow: 'hidden', background: '#111' }}>
                    <Thumbnail thumbnailPath={thumbnailPaths[path] ?? null} hasError={Boolean(thumbnailErrors[path])} />
                  </div>
                  <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 8, flex: 1, justifyContent: 'center' }}>
                    <button className="btn btn-secondary btn-compact" style={{ width: '100%' }} onClick={() => applyWallpaperOnly(path)} title="Sets the wallpaper in KDE only">
                      {t('source.applyOnly')}
                    </button>
                    <button className="btn btn-primary btn-compact" style={{ width: '100%' }} onClick={() => handleApplyAndGenerate(path)} title="Sets wallpaper, generates colors, and updates all templates">
                      {t('source.applyAndGenerate')}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
          <div style={{ padding: '12px 16px 0', display: 'flex', justifyContent: 'center' }}>
            <div style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: 12, maxWidth: '100%', flexWrap: 'nowrap' }}>
              <CompactPaginationButton
                label={t('source.previousPage')}
                title={t('source.previousPage')}
                disabled={page.currentPage <= 1 || isScanningSource}
                onClick={() => goToPage(page.currentPage - 1)}
              >
                <ChevronLeft size={16} />
                <span className="pagination-button-label">{t('source.previous')}</span>
              </CompactPaginationButton>
              <div style={{ color: 'var(--text-secondary)', fontSize: 13, fontWeight: 500, whiteSpace: 'nowrap', minWidth: 112, textAlign: 'center' }}>
                {t('source.pageOf', { current: page.currentPage, total: page.totalPages })}
              </div>
              <CompactPaginationButton
                label={t('source.nextPage')}
                title={t('source.nextPage')}
                disabled={page.currentPage >= page.totalPages || isScanningSource}
                onClick={() => goToPage(page.currentPage + 1)}
              >
                <span className="pagination-button-label">{t('source.next')}</span>
                <ChevronRight size={16} />
              </CompactPaginationButton>
            </div>
          </div>
        </>
      )}

      <style>
        {`
          @media (max-width: 520px) {
            .pagination-button-label {
              display: none;
            }
          }
        `}
      </style>
    </div>
  );
}
