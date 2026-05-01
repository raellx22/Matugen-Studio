export const DEFAULT_WALLPAPERS_PER_PAGE = 20;
export const MIN_WALLPAPERS_PER_PAGE = 10;
export const MAX_WALLPAPERS_PER_PAGE = 50;
export const WALLPAPERS_PER_PAGE_OPTIONS = [10, 20, 30, 40, 50] as const;

export interface WallpaperPage<T> {
  currentPage: number;
  items: T[];
  totalItems: number;
  totalPages: number;
  startIndex: number;
  endIndex: number;
}

export function validateWallpapersPerPage(value: unknown): number {
  const parsed = typeof value === 'number' ? value : Number(value);

  if (!Number.isInteger(parsed)) {
    return DEFAULT_WALLPAPERS_PER_PAGE;
  }

  if (parsed < MIN_WALLPAPERS_PER_PAGE || parsed > MAX_WALLPAPERS_PER_PAGE) {
    return DEFAULT_WALLPAPERS_PER_PAGE;
  }

  return parsed;
}

export function getWallpaperPage<T>(
  items: T[],
  currentPage: number,
  itemsPerPage: number,
): WallpaperPage<T> {
  const safeItemsPerPage = validateWallpapersPerPage(itemsPerPage);
  const totalItems = items.length;
  const totalPages = Math.max(1, Math.ceil(totalItems / safeItemsPerPage));
  const safeCurrentPage = Math.min(Math.max(1, currentPage), totalPages);
  const startIndex = (safeCurrentPage - 1) * safeItemsPerPage;
  const endIndex = Math.min(startIndex + safeItemsPerPage, totalItems);

  return {
    currentPage: safeCurrentPage,
    items: items.slice(startIndex, endIndex),
    totalItems,
    totalPages,
    startIndex,
    endIndex,
  };
}
