import { invoke } from '@tauri-apps/api/core';
export type SourceKind = 'user_folder' | 'matugen_downloads' | 'download_archive' | 'kde_user' | 'kde_system' | 'system_backgrounds' | 'current_wallpaper';
export interface WallpaperSource { id: string; kind: SourceKind; path: string; enabled: boolean; recursive: boolean; removable: boolean; displayName: string }
export interface ProviderRecord { provider: string; providerId: string; cachePath: string | null; activePath: string | null; savedPath: string | null; thumb: string | null; lastUsedAt: number | null; savedAt: number | null; hiddenFromHistory: boolean }
export interface WallpaperLibrary { downloadRoot: string; sources: WallpaperSource[]; providerRecords: Record<string, ProviderRecord>; legacyFoldersMigrated: boolean; legacyHistoryMigrated: boolean }
export interface WallpaperItem { path: string; fileName: string; sourceId: string; sourceKind: SourceKind; provider: string | null; providerId: string | null }
export interface ScanResult { items: WallpaperItem[]; errors: { sourceId: string; message: string }[] }
export const scanWallpaperLibrary = () => invoke<ScanResult>('rescan_wallpaper_library');
export async function initializeWallpaperLibrary(): Promise<WallpaperLibrary> {
  const legacyFolders = Object.keys(localStorage).filter(key => /^wallpaperFolder_-?\d+$/.test(key)).map(key => localStorage.getItem(key)).filter((path): path is string => Boolean(path));
  let legacyHistory: { id: string; localPath: string; thumb?: string; downloadedAt?: number }[] = [];
  try { const value = JSON.parse(localStorage.getItem('wallhavenDownloadHistory') || '[]'); if (Array.isArray(value)) legacyHistory = value.filter(entry => entry && typeof entry.id === 'string' && typeof entry.localPath === 'string'); } catch { /* keep the legacy key untouched */ }
  return invoke<WallpaperLibrary>('initialize_wallpaper_library', { legacyFolders, legacyHistory });
}
