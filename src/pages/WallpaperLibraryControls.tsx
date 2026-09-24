import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import type { WallpaperLibrary, WallpaperSource } from '../utils/wallpaperLibrary';
import { scanWallpaperLibrary } from '../utils/wallpaperLibrary';
import { FolderOpen } from 'lucide-react';
import { Section, SettingRow, Switch } from '../components/Primitives';

interface Props { onChanged?: () => void }
export default function WallpaperLibraryControls({ onChanged }: Props) {
  const { t } = useTranslation();
  const [library, setLibrary] = useState<WallpaperLibrary | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { void invoke<WallpaperLibrary>('get_wallpaper_library').then(setLibrary).catch(e => setError(String(e))); }, []);
  const change = async (command: string, args: Record<string, unknown> = {}) => {
    setBusy(true); setError(null);
    try { setLibrary(await invoke<WallpaperLibrary>(command, args)); onChanged?.(); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  const pickFolder = async (root: boolean) => {
    const path = await open({ directory: true, multiple: false });
    if (typeof path !== 'string') return;
    if (root) await change('set_wallpaper_download_root', { path });
    else await change('add_wallpaper_source', { path });
  };
  const rescan = async () => { setBusy(true); try { await scanWallpaperLibrary(); onChanged?.(); } catch(e) { setError(String(e)); } finally { setBusy(false); } };
  const label = (source: WallpaperSource) => t(`library.kind.${source.kind}`);
  const name = (source: WallpaperSource) => source.kind === 'user_folder' ? (source.path.split(/[\\/]/).filter(Boolean).pop() ?? label(source)) : label(source);
  return <Section title={t('library.title')} className="wallpaper-library-controls">
    <SettingRow title={t('library.downloadFolder')} description={library?.downloadRoot ?? t('library.loading')}><button className="btn btn-secondary btn-compact" disabled={busy} onClick={() => void pickFolder(true)}>{t('library.change')}</button></SettingRow>
    <div className="library-source-list">{library?.sources.map(source => <div className="library-source-row" key={source.id}>
      <FolderOpen size={18} className="library-source-icon" aria-hidden="true" />
      <span className="library-source-copy"><strong>{name(source)}</strong><small>{label(source)} · {source.path}</small></span>
      <Switch label={name(source)} checked={source.enabled} disabled={busy} onChange={enabled => void change('set_wallpaper_source_enabled', { id: source.id, enabled })} />
      {source.removable && <button className="btn btn-secondary btn-compact" disabled={busy} onClick={() => void change('remove_wallpaper_source', { id: source.id })}>{t('library.remove')}</button>}
    </div>)}</div>
    <div className="library-source-actions"><button className="btn btn-secondary btn-compact" disabled={busy} onClick={() => void pickFolder(false)}>{t('library.addFolder')}</button><button className="btn btn-ghost btn-compact" disabled={busy} onClick={() => void rescan()}>{t('library.rescan')}</button></div>
    {error && <p role="alert" style={{ color: 'var(--danger)' }}>{error}</p>}
  </Section>;
}
