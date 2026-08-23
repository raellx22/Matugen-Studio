import { useEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { BookmarkPlus, Check, ChevronDown, ChevronLeft, ChevronRight, ChevronUp, Download, History, Palette, RotateCcw, Search, SlidersHorizontal, Sparkles, Trash2, X } from 'lucide-react';
import { CompactPaginationButton } from './Source';
import { useSessionState } from '../utils/sessionState';

const NSFW_DISCLAIMER_STORAGE_KEY = 'wallhavenNsfwDisclaimerAcknowledged';
const HISTORY_STORAGE_KEY = 'wallhavenDownloadHistory';
const HISTORY_LIMIT = 60;
const DEFAULT_FILTERS_STORAGE_KEY = 'wallhavenDefaultFilters';

interface WallhavenBrowserProps {
  onApplyAndGenerate: (path: string) => Promise<void>;
  onSelectForColors: (path: string) => void;
  themeColorHex?: string | null;
}

interface WallhavenThumbs {
  large: string;
  original: string;
  small: string;
}

interface WallhavenTag {
  id: number;
  name: string;
  alias: string;
  categoryId: number;
  category: string;
  purity: string;
}

interface WallhavenUploader {
  username: string;
  group: string;
}

interface WallhavenWallpaper {
  id: string;
  purity: string;
  category: string;
  resolution: string;
  fileSize: number;
  path: string;
  thumbs: WallhavenThumbs;
  tags?: WallhavenTag[] | null;
  uploader?: WallhavenUploader | null;
}

interface WallhavenMeta {
  currentPage: number;
  lastPage: number;
  total: number;
  seed: string | null;
}

interface WallhavenSearchResponse {
  data: WallhavenWallpaper[];
  meta: WallhavenMeta;
}

interface HistoryEntry {
  id: string;
  localPath: string;
  thumb: string;
  downloadedAt: number;
}

type SortingOption = 'date_added' | 'relevance' | 'random' | 'views' | 'favorites' | 'toplist';
type OrderOption = 'desc' | 'asc';
type DownloadState = 'idle' | 'downloading' | 'done' | 'error';
type TopRangeOption = '1d' | '3d' | '1w' | '1M' | '3M' | '6M' | '1y';

const SORTING_OPTIONS: SortingOption[] = ['date_added', 'relevance', 'random', 'views', 'favorites', 'toplist'];
const TOP_RANGE_OPTIONS: TopRangeOption[] = ['1d', '3d', '1w', '1M', '3M', '6M', '1y'];
const RATIO_OPTIONS = ['16x9', '16x10', '21x9', '32x9', '4x3', '5x4', '3x2', '1x1'];
const CATEGORY_KEYS = ['general', 'anime', 'people'] as const;
type CategoryKey = (typeof CATEGORY_KEYS)[number];

interface WallhavenFilterPreset {
  categories: Record<CategoryKey, boolean>;
  sketchy: boolean;
  nsfw: boolean;
  sorting: SortingOption;
  order: OrderOption;
  topRange: TopRangeOption;
  colorFilter: string | null;
  selectedRatios: string[];
  atleast: string;
}

const FACTORY_FILTERS: WallhavenFilterPreset = {
  categories: { general: true, anime: true, people: true },
  sketchy: false,
  nsfw: false,
  sorting: 'date_added',
  order: 'desc',
  topRange: '1M',
  colorFilter: null,
  selectedRatios: [],
  atleast: '',
};

// The saved default is a long-lived user preference (survives app restarts),
// distinct from the in-session filter state tracked via `useSessionState`.
function loadDefaultFilters(): WallhavenFilterPreset {
  const raw = localStorage.getItem(DEFAULT_FILTERS_STORAGE_KEY);
  if (!raw) return FACTORY_FILTERS;
  try {
    return { ...FACTORY_FILTERS, ...JSON.parse(raw) };
  } catch {
    return FACTORY_FILTERS;
  }
}

// Fixed swatch list accepted by Wallhaven's `colors` search parameter.
const COLOR_SWATCHES = [
  '660000', '990000', 'cc0000', 'cc3333', 'ea4c88', '993399', '663399', '333399',
  '0066cc', '0099cc', '66cccc', '77cc33', '669900', '336600', '666600', '999900',
  'cccc33', 'ffff00', 'ffcc33', 'ff9900', 'ff6600', 'cc6633', '996633', '663300',
  '000000', '999999', 'cccccc', 'ffffff', '424153',
];

const selectStyle: CSSProperties = {
  padding: '8px 12px',
  borderRadius: 8,
  background: 'var(--surface-hover)',
  border: '1px solid var(--border)',
  color: 'var(--text-primary)',
};

const pillStyle: CSSProperties = {
  background: 'var(--surface-hover)',
  border: '1px solid var(--border)',
  borderRadius: 999,
  padding: '4px 10px',
  fontSize: 12,
  color: 'var(--text-secondary)',
};

function hexToRgb(hex: string): [number, number, number] {
  const clean = hex.replace('#', '');
  return [
    parseInt(clean.slice(0, 2), 16),
    parseInt(clean.slice(2, 4), 16),
    parseInt(clean.slice(4, 6), 16),
  ];
}

function nearestSwatch(hex: string): string {
  const [r, g, b] = hexToRgb(hex);
  let closest = COLOR_SWATCHES[0];
  let closestDistance = Infinity;
  for (const swatch of COLOR_SWATCHES) {
    const [sr, sg, sb] = hexToRgb(swatch);
    const distance = (r - sr) ** 2 + (g - sg) ** 2 + (b - sb) ** 2;
    if (distance < closestDistance) {
      closestDistance = distance;
      closest = swatch;
    }
  }
  return closest;
}

function formatFileSize(bytes: number): string {
  if (!bytes) return '';
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value.toFixed(1)} ${units[unitIndex]}`;
}

function loadHistory(): HistoryEntry[] {
  try {
    const raw = localStorage.getItem(HISTORY_STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function saveHistory(entries: HistoryEntry[]) {
  localStorage.setItem(HISTORY_STORAGE_KEY, JSON.stringify(entries));
}

export default function WallhavenBrowser({ onApplyAndGenerate, onSelectForColors, themeColorHex }: WallhavenBrowserProps) {
  const { t } = useTranslation();
  const defaultFilters = useMemo(() => loadDefaultFilters(), []);
  const [query, setQuery] = useSessionState('wallhaven.query', '');
  const [debouncedQuery, setDebouncedQuery] = useSessionState('wallhaven.debouncedQuery', '');
  const [categories, setCategories] = useSessionState<Record<CategoryKey, boolean>>('wallhaven.categories', defaultFilters.categories);
  const [sketchy, setSketchy] = useSessionState('wallhaven.sketchy', defaultFilters.sketchy);
  const [nsfw, setNsfw] = useSessionState('wallhaven.nsfw', () => defaultFilters.nsfw && Boolean(localStorage.getItem('wallhavenApiKey')));
  const [showNsfwModal, setShowNsfwModal] = useState(false);
  const [apiKey] = useState(() => localStorage.getItem('wallhavenApiKey') || '');
  const [sorting, setSorting] = useSessionState<SortingOption>('wallhaven.sorting', defaultFilters.sorting);
  const [order, setOrder] = useSessionState<OrderOption>('wallhaven.order', defaultFilters.order);
  const [topRange, setTopRange] = useSessionState<TopRangeOption>('wallhaven.topRange', defaultFilters.topRange);
  const [colorFilter, setColorFilter] = useSessionState<string | null>('wallhaven.colorFilter', defaultFilters.colorFilter);
  const [selectedRatios, setSelectedRatios] = useSessionState<Set<string>>('wallhaven.selectedRatios', () => new Set(defaultFilters.selectedRatios));
  const [atleast, setAtleast] = useSessionState('wallhaven.atleast', defaultFilters.atleast);
  const [debouncedAtleast, setDebouncedAtleast] = useSessionState('wallhaven.debouncedAtleast', defaultFilters.atleast);
  const [page, setPage] = useSessionState('wallhaven.page', 1);
  const [seed, setSeed] = useSessionState<string | null>('wallhaven.seed', null);
  const [results, setResults] = useState<WallhavenWallpaper[]>([]);
  const [meta, setMeta] = useState<WallhavenMeta | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [downloadState, setDownloadState] = useState<Record<string, DownloadState>>(() => {
    const map: Record<string, DownloadState> = {};
    for (const entry of loadHistory()) map[entry.id] = 'done';
    return map;
  });
  const [localPaths, setLocalPaths] = useState<Record<string, string>>(() => {
    const map: Record<string, string> = {};
    for (const entry of loadHistory()) map[entry.id] = entry.localPath;
    return map;
  });
  const [history, setHistory] = useState<HistoryEntry[]>(() => loadHistory());
  const [showHistory, setShowHistory] = useSessionState('wallhaven.showHistory', false);
  const [filtersOpen, setFiltersOpen] = useSessionState('wallhaven.filtersOpen', true);
  const [hasSavedFilters, setHasSavedFilters] = useState(() => localStorage.getItem(DEFAULT_FILTERS_STORAGE_KEY) !== null);
  const [filtersSaved, setFiltersSaved] = useState(false);
  const [detailItem, setDetailItem] = useState<WallhavenWallpaper | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);

  const requestGenerationRef = useRef(0);
  const isMountRestoreRef = useRef(true);

  useEffect(() => {
    const handle = setTimeout(() => setDebouncedQuery(query), 450);
    return () => clearTimeout(handle);
  }, [query]);

  useEffect(() => {
    const handle = setTimeout(() => setDebouncedAtleast(atleast), 450);
    return () => clearTimeout(handle);
  }, [atleast]);

  useEffect(() => {
    // On mount, `debouncedQuery`/`debouncedAtleast` may already equal the
    // restored session values (no real change), but this effect still runs
    // once after every mount regardless of the dependency diff. Skip that
    // first run so returning to an already-paginated search doesn't snap
    // back to page 1.
    if (isMountRestoreRef.current) {
      isMountRestoreRef.current = false;
      return;
    }
    setPage(1);
  }, [debouncedQuery, debouncedAtleast]);

  useEffect(() => {
    const generation = ++requestGenerationRef.current;
    setIsLoading(true);
    setErrorMsg(null);

    const categoriesBitmask = `${categories.general ? 1 : 0}${categories.anime ? 1 : 0}${categories.people ? 1 : 0}`;
    const purityBitmask = `1${sketchy ? 1 : 0}${nsfw ? 1 : 0}`;
    const ratiosParam = selectedRatios.size > 0 ? Array.from(selectedRatios).join(',') : undefined;

    invoke<WallhavenSearchResponse>('wallhaven_search', {
      params: {
        q: debouncedQuery.trim() || undefined,
        categories: categoriesBitmask,
        purity: purityBitmask,
        sorting,
        order,
        topRange: sorting === 'toplist' ? topRange : undefined,
        colors: colorFilter ?? undefined,
        ratios: ratiosParam,
        atleast: debouncedAtleast.trim() || undefined,
        page,
        seed: sorting === 'random' ? seed ?? undefined : undefined,
        apiKey: apiKey || undefined,
      },
    })
      .then((response) => {
        if (requestGenerationRef.current !== generation) return;
        setResults(response.data);
        setMeta(response.meta);
        if (sorting === 'random' && response.meta.seed) {
          setSeed(response.meta.seed);
        }
      })
      .catch((e) => {
        if (requestGenerationRef.current !== generation) return;
        setErrorMsg(String(e));
        setResults([]);
        setMeta(null);
      })
      .finally(() => {
        if (requestGenerationRef.current === generation) setIsLoading(false);
      });
    // `seed` is intentionally excluded: it is produced by the response above and
    // must not itself retrigger a fetch (would refire on every random page load).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [debouncedQuery, debouncedAtleast, categories, sketchy, nsfw, sorting, order, topRange, colorFilter, selectedRatios, page]);

  const toggleCategory = (key: CategoryKey) => {
    setCategories((prev) => {
      const next = { ...prev, [key]: !prev[key] };
      if (!next.general && !next.anime && !next.people) return prev;
      return next;
    });
    setPage(1);
  };

  const toggleRatio = (ratio: string) => {
    setSelectedRatios((prev) => {
      const next = new Set(prev);
      if (next.has(ratio)) {
        next.delete(ratio);
      } else {
        next.add(ratio);
      }
      return next;
    });
    setPage(1);
  };

  const handleColorSelect = (hex: string) => {
    setColorFilter((prev) => (prev === hex ? null : hex));
    setPage(1);
  };

  const useThemeColor = () => {
    if (!themeColorHex) return;
    setColorFilter(nearestSwatch(themeColorHex));
    setPage(1);
  };

  const saveDefaultFilters = () => {
    const preset: WallhavenFilterPreset = {
      categories,
      sketchy,
      nsfw,
      sorting,
      order,
      topRange,
      colorFilter,
      selectedRatios: Array.from(selectedRatios),
      atleast,
    };
    localStorage.setItem(DEFAULT_FILTERS_STORAGE_KEY, JSON.stringify(preset));
    setHasSavedFilters(true);
    setFiltersSaved(true);
    setTimeout(() => setFiltersSaved(false), 2000);
  };

  const clearDefaultFilters = () => {
    localStorage.removeItem(DEFAULT_FILTERS_STORAGE_KEY);
    setHasSavedFilters(false);
  };

  const enableNsfw = () => {
    localStorage.setItem(NSFW_DISCLAIMER_STORAGE_KEY, 'true');
    setNsfw(true);
    setPage(1);
    setShowNsfwModal(false);
  };

  const handleNsfwToggle = () => {
    if (nsfw) {
      setNsfw(false);
      setPage(1);
      return;
    }
    if (!apiKey) return;
    if (localStorage.getItem(NSFW_DISCLAIMER_STORAGE_KEY) === 'true') {
      setNsfw(true);
      setPage(1);
    } else {
      setShowNsfwModal(true);
    }
  };

  const addToHistory = (id: string, localPath: string, thumb: string) => {
    setHistory((prev) => {
      const next = [{ id, localPath, thumb, downloadedAt: Date.now() }, ...prev.filter((entry) => entry.id !== id)].slice(0, HISTORY_LIMIT);
      saveHistory(next);
      return next;
    });
  };

  const removeFromHistory = (id: string) => {
    setHistory((prev) => {
      const next = prev.filter((entry) => entry.id !== id);
      saveHistory(next);
      return next;
    });
  };

  const clearHistory = () => {
    setHistory([]);
    saveHistory([]);
  };

  const downloadWallpaper = async (item: WallhavenWallpaper): Promise<string | null> => {
    if (localPaths[item.id]) return localPaths[item.id];
    setDownloadState((prev) => ({ ...prev, [item.id]: 'downloading' }));
    try {
      const localPath = await invoke<string>('wallhaven_download', { id: item.id, url: item.path });
      setLocalPaths((prev) => ({ ...prev, [item.id]: localPath }));
      setDownloadState((prev) => ({ ...prev, [item.id]: 'done' }));
      addToHistory(item.id, localPath, item.thumbs.small);
      return localPath;
    } catch (e) {
      setDownloadState((prev) => ({ ...prev, [item.id]: 'error' }));
      alert(t('wallhaven.downloadFailed', { error: String(e) }));
      return null;
    }
  };

  const handleApply = async (item: WallhavenWallpaper) => {
    const localPath = await downloadWallpaper(item);
    if (localPath) {
      await onApplyAndGenerate(localPath);
    }
  };

  const handleSelectForColors = async (item: WallhavenWallpaper) => {
    const localPath = await downloadWallpaper(item);
    if (localPath) {
      onSelectForColors(localPath);
    }
  };

  const openDetail = async (item: WallhavenWallpaper) => {
    setDetailItem(item);
    setDetailError(null);
    setDetailLoading(true);
    try {
      const full = await invoke<WallhavenWallpaper>('wallhaven_get_wallpaper', { id: item.id, apiKey: apiKey || undefined });
      setDetailItem(full);
    } catch (e) {
      setDetailError(String(e));
    } finally {
      setDetailLoading(false);
    }
  };

  const handleShowSimilar = (id: string) => {
    setDetailItem(null);
    setShowHistory(false);
    setQuery(`like:${id}`);
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16, flex: 1, minHeight: 0 }}>
      <div style={{ background: 'var(--surface)', padding: 16, borderRadius: 16, border: '1px solid var(--border)', display: 'flex', flexDirection: 'column', gap: 12 }}>
        <div style={{ display: 'flex', gap: 8 }}>
          <div style={{ position: 'relative', flex: 1 }}>
            <Search size={16} style={{ position: 'absolute', left: 12, top: '50%', transform: 'translateY(-50%)', color: 'var(--text-muted)' }} />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t('wallhaven.searchPlaceholder')}
              style={{ width: '100%', padding: '10px 12px 10px 36px', borderRadius: 'var(--radius-sm)', background: 'var(--surface-hover)', border: '1px solid var(--border)', color: 'var(--text-primary)' }}
            />
          </div>
          <button
            type="button"
            className={`btn btn-compact ${showHistory ? 'btn-primary' : 'btn-secondary'}`}
            onClick={() => setShowHistory((prev) => !prev)}
          >
            <History size={14} /> {t('wallhaven.history')}{history.length > 0 ? ` (${history.length})` : ''}
          </button>
          {!showHistory && (
            <button
              type="button"
              className={`btn btn-compact ${filtersOpen ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setFiltersOpen((prev) => !prev)}
              title={filtersOpen ? t('wallhaven.hideFilters') : t('wallhaven.showFilters')}
            >
              <SlidersHorizontal size={14} /> {t('wallhaven.filters')} {filtersOpen ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
            </button>
          )}
        </div>

        {!showHistory && filtersOpen && (
        <>
        <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap', alignItems: 'center' }}>
          <div style={{ display: 'flex', gap: 8 }}>
            {CATEGORY_KEYS.map((key) => (
              <button
                key={key}
                type="button"
                className={`btn btn-compact ${categories[key] ? 'btn-primary' : 'btn-secondary'}`}
                onClick={() => toggleCategory(key)}
              >
                {t(`wallhaven.category.${key}`)}
              </button>
            ))}
          </div>
          <button
            type="button"
            className={`btn btn-compact ${sketchy ? 'btn-primary' : 'btn-secondary'}`}
            onClick={() => { setSketchy((prev) => !prev); setPage(1); }}
            title={t('wallhaven.sketchyHint')}
          >
            {t('wallhaven.sketchy')}
          </button>
          <button
            type="button"
            className={`btn btn-compact ${nsfw ? 'btn-primary' : 'btn-secondary'}`}
            onClick={handleNsfwToggle}
            disabled={!apiKey}
            title={apiKey ? t('wallhaven.nsfwHint') : t('wallhaven.nsfwNeedsKey')}
            style={!apiKey ? { opacity: 0.5, cursor: 'not-allowed' } : undefined}
          >
            {t('wallhaven.nsfw')}
          </button>
          <select
            value={sorting}
            onChange={(e) => { setSorting(e.target.value as SortingOption); setPage(1); setSeed(null); }}
            style={selectStyle}
          >
            {SORTING_OPTIONS.map((option) => (
              <option key={option} value={option}>{t(`wallhaven.sorting.${option}`)}</option>
            ))}
          </select>
          {sorting === 'toplist' && (
            <select
              value={topRange}
              onChange={(e) => { setTopRange(e.target.value as TopRangeOption); setPage(1); }}
              style={selectStyle}
            >
              {TOP_RANGE_OPTIONS.map((option) => (
                <option key={option} value={option}>{t(`wallhaven.topRange.${option}`)}</option>
              ))}
            </select>
          )}
          <select
            value={order}
            onChange={(e) => { setOrder(e.target.value as OrderOption); setPage(1); }}
            style={selectStyle}
          >
            <option value="desc">{t('wallhaven.orderDesc')}</option>
            <option value="asc">{t('wallhaven.orderAsc')}</option>
          </select>
        </div>

        <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap', alignItems: 'center' }}>
          <span style={{ fontSize: 12, color: 'var(--text-secondary)' }}>{t('wallhaven.colorsLabel')}</span>
          <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
            {COLOR_SWATCHES.map((hex) => (
              <button
                key={hex}
                type="button"
                onClick={() => handleColorSelect(hex)}
                title={`#${hex}`}
                style={{
                  width: 20,
                  height: 20,
                  borderRadius: '50%',
                  background: `#${hex}`,
                  border: colorFilter === hex ? '2px solid var(--accent)' : '1px solid var(--border-strong)',
                  boxShadow: colorFilter === hex ? '0 0 0 2px var(--accent-soft)' : 'none',
                  cursor: 'pointer',
                  padding: 0,
                }}
              />
            ))}
          </div>
          {themeColorHex && (
            <button type="button" className="btn btn-secondary btn-compact" onClick={useThemeColor}>
              <Palette size={14} /> {t('wallhaven.useThemeColor')}
            </button>
          )}
        </div>

        <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap', alignItems: 'center' }}>
          <span style={{ fontSize: 12, color: 'var(--text-secondary)' }}>{t('wallhaven.ratiosLabel')}</span>
          <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
            {RATIO_OPTIONS.map((ratio) => (
              <button
                key={ratio}
                type="button"
                className={`btn btn-compact ${selectedRatios.has(ratio) ? 'btn-primary' : 'btn-secondary'}`}
                onClick={() => toggleRatio(ratio)}
              >
                {ratio.replace('x', ':')}
              </button>
            ))}
          </div>
          <input
            type="text"
            value={atleast}
            onChange={(e) => setAtleast(e.target.value)}
            placeholder={t('wallhaven.atleastPlaceholder')}
            style={{ width: 200, padding: '6px 10px', borderRadius: 8, background: 'var(--surface-hover)', border: '1px solid var(--border)', color: 'var(--text-primary)', fontSize: 13 }}
          />
        </div>

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', paddingTop: 8, borderTop: '1px solid var(--border)' }}>
          {hasSavedFilters && (
            <button type="button" className="btn btn-secondary btn-compact" onClick={clearDefaultFilters} title={t('wallhaven.clearDefaultFilters')}>
              <RotateCcw size={14} /> {t('wallhaven.clearDefaultFilters')}
            </button>
          )}
          <button type="button" className="btn btn-secondary btn-compact" onClick={saveDefaultFilters} title={t('wallhaven.saveDefaultFiltersHint')}>
            {filtersSaved ? (<><Check size={14} /> {t('wallhaven.defaultFiltersSaved')}</>) : (<><BookmarkPlus size={14} /> {t('wallhaven.saveDefaultFilters')}</>)}
          </button>
        </div>
        </>
        )}
      </div>

      {showHistory ? (
        history.length === 0 ? (
          <p>{t('wallhaven.historyEmpty')}</p>
        ) : (
          <>
            <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
              <button className="btn btn-secondary btn-compact" onClick={clearHistory}>
                <Trash2 size={14} /> {t('wallhaven.clearHistory')}
              </button>
            </div>
            <div
              className="wallpaper-grid"
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
              {history.map((entry) => (
                <div key={entry.id} className="template-card" style={{
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
                      style={{ padding: 0, background: 'rgba(0,0,0,0.5)', border: '1px solid rgba(255,255,255,0.1)', color: '#fff' }}
                      onClick={() => removeFromHistory(entry.id)}
                      title={t('wallhaven.removeFromHistory')}
                    >
                      <X size={18} />
                    </button>
                  </div>
                  <div className="template-thumb" style={{ height: 160, width: '100%', overflow: 'hidden', background: '#111' }}>
                    <img className="template-thumb-img" src={entry.thumb} alt={entry.id} loading="lazy" decoding="async" style={{ width: '100%', height: '100%', objectFit: 'cover' }} />
                  </div>
                  <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 8, flex: 1, justifyContent: 'center' }}>
                    <button
                      className="btn btn-secondary btn-compact"
                      style={{ width: '100%' }}
                      onClick={() => onSelectForColors(entry.localPath)}
                    >
                      {t('source.loadColors')}
                    </button>
                    <button
                      className="btn btn-primary btn-compact"
                      style={{ width: '100%' }}
                      onClick={() => onApplyAndGenerate(entry.localPath)}
                    >
                      {t('source.applyAndGenerate')}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </>
        )
      ) : (
        <>
          {errorMsg && <div style={{ color: 'var(--danger)', fontSize: 13 }}>{errorMsg}</div>}

          {isLoading && results.length === 0 ? (
            <div style={{ flex: 1, display: 'flex', justifyContent: 'center', alignItems: 'center' }}>
              <p>{t('wallhaven.loading')}</p>
            </div>
          ) : results.length === 0 ? (
            <p>{t('wallhaven.noResults')}</p>
          ) : (
            <>
              <div
                className="wallpaper-grid"
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))',
                  gap: 16,
                  overflowY: 'auto',
                  paddingRight: 8,
                  paddingBottom: 24,
                  flex: 1,
                  opacity: isLoading ? 0.6 : 1,
                  transition: 'opacity 0.2s ease',
                }}
              >
                {results.map((item) => {
                  const state = downloadState[item.id] ?? 'idle';
                  return (
                    <div key={item.id} className="template-card" style={{
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
                          style={{ padding: 0, background: 'rgba(0,0,0,0.5)', border: '1px solid rgba(255,255,255,0.1)', color: '#fff' }}
                          onClick={() => handleSelectForColors(item)}
                          title={t('source.loadColors')}
                        >
                          <Palette size={18} />
                        </button>
                      </div>
                      <div style={{ position: 'absolute', top: 8, left: 8, zIndex: 10, display: 'flex', gap: 6 }}>
                        <span style={{ background: 'rgba(0,0,0,0.55)', color: '#fff', fontSize: 11, padding: '3px 8px', borderRadius: 999 }}>
                          {item.resolution}
                        </span>
                        {item.purity !== 'sfw' && (
                          <span style={{ background: 'rgba(0,0,0,0.55)', color: '#ffb4ab', fontSize: 11, padding: '3px 8px', borderRadius: 999 }}>
                            {item.purity.toUpperCase()}
                          </span>
                        )}
                      </div>
                      <div
                        className="template-thumb"
                        style={{ height: 160, width: '100%', overflow: 'hidden', background: '#111', cursor: 'pointer' }}
                        onClick={() => openDetail(item)}
                        title={t('wallhaven.viewDetails')}
                      >
                        <img
                          className="template-thumb-img"
                          src={item.thumbs.small}
                          alt={item.id}
                          loading="lazy"
                          decoding="async"
                          style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                        />
                      </div>
                      <div style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 8, flex: 1, justifyContent: 'center' }}>
                        <button
                          className="btn btn-secondary btn-compact"
                          style={{ width: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6 }}
                          onClick={() => downloadWallpaper(item)}
                          disabled={state === 'downloading'}
                        >
                          {state === 'downloading' ? (
                            t('wallhaven.downloading')
                          ) : state === 'done' ? (
                            <><Check size={14} /> {t('wallhaven.downloaded')}</>
                          ) : (
                            <><Download size={14} /> {t('wallhaven.downloadOnly')}</>
                          )}
                        </button>
                        <button
                          className="btn btn-primary btn-compact"
                          style={{ width: '100%' }}
                          onClick={() => handleApply(item)}
                          disabled={state === 'downloading'}
                        >
                          {t('source.applyAndGenerate')}
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>

              {meta && (
                <div style={{ padding: '12px 16px 0', display: 'flex', justifyContent: 'center' }}>
                  <div style={{ display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: 12 }}>
                    <CompactPaginationButton
                      label={t('source.previousPage')}
                      title={t('source.previousPage')}
                      disabled={meta.currentPage <= 1 || isLoading}
                      onClick={() => setPage((p) => Math.max(1, p - 1))}
                    >
                      <ChevronLeft size={16} />
                      <span className="pagination-button-label">{t('source.previous')}</span>
                    </CompactPaginationButton>
                    <div style={{ color: 'var(--text-secondary)', fontSize: 13, fontWeight: 500, minWidth: 112, textAlign: 'center' }}>
                      {t('source.pageOf', { current: meta.currentPage, total: meta.lastPage })}
                    </div>
                    <CompactPaginationButton
                      label={t('source.nextPage')}
                      title={t('source.nextPage')}
                      disabled={meta.currentPage >= meta.lastPage || isLoading}
                      onClick={() => setPage((p) => p + 1)}
                    >
                      <span className="pagination-button-label">{t('source.next')}</span>
                      <ChevronRight size={16} />
                    </CompactPaginationButton>
                  </div>
                </div>
              )}
            </>
          )}
        </>
      )}

      {showNsfwModal && (
        <div className="modal-overlay" onClick={() => setShowNsfwModal(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 460 }}>
            <h2 style={{ marginTop: 0 }}>{t('wallhaven.nsfwDisclaimerTitle')}</h2>
            <p style={{ color: 'var(--text-secondary)', fontSize: 14, lineHeight: 1.5 }}>
              {t('wallhaven.nsfwDisclaimerBody')}
            </p>
            <div className="modal-actions">
              <button className="btn btn-secondary" onClick={() => setShowNsfwModal(false)}>
                {t('presets.cancel')}
              </button>
              <button className="btn btn-primary" onClick={enableNsfw}>
                {t('wallhaven.nsfwDisclaimerConfirm')}
              </button>
            </div>
          </div>
        </div>
      )}

      {detailItem && (
        <div className="modal-overlay" onClick={() => setDetailItem(null)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 640, maxHeight: '85vh', overflowY: 'auto' }}>
            <img
              src={detailItem.thumbs.large}
              alt={detailItem.id}
              style={{ width: '100%', borderRadius: 12, marginBottom: 16, display: 'block' }}
            />
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: 12 }}>
              <span style={pillStyle}>{detailItem.resolution}</span>
              <span style={pillStyle}>{detailItem.category}</span>
              <span style={pillStyle}>{detailItem.purity.toUpperCase()}</span>
              {detailItem.fileSize > 0 && <span style={pillStyle}>{formatFileSize(detailItem.fileSize)}</span>}
              {detailItem.uploader && <span style={pillStyle}>@{detailItem.uploader.username}</span>}
            </div>

            {detailLoading && <p style={{ color: 'var(--text-secondary)', fontSize: 13 }}>{t('wallhaven.loadingDetails')}</p>}
            {detailError && <p style={{ color: 'var(--danger)', fontSize: 13 }}>{detailError}</p>}

            {detailItem.tags && detailItem.tags.length > 0 && (
              <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginBottom: 16 }}>
                {detailItem.tags.map((tag) => (
                  <span key={tag.id} style={pillStyle}>{tag.name}</span>
                ))}
              </div>
            )}

            <div className="modal-actions" style={{ flexWrap: 'wrap' }}>
              <button className="btn btn-secondary" onClick={() => downloadWallpaper(detailItem)}>
                <Download size={14} /> {t('wallhaven.downloadOnly')}
              </button>
              <button className="btn btn-secondary" onClick={() => handleSelectForColors(detailItem)}>
                <Palette size={14} /> {t('source.loadColors')}
              </button>
              <button className="btn btn-secondary" onClick={() => handleShowSimilar(detailItem.id)}>
                <Sparkles size={14} /> {t('wallhaven.showSimilar')}
              </button>
              <button className="btn btn-primary" onClick={() => handleApply(detailItem)}>
                {t('source.applyAndGenerate')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
