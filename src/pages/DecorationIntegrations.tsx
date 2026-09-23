import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import type { KdeIntegrationSettings } from "../utils/studioSettings";
interface Detection { detected: boolean; supported: boolean; version: string | null; active: boolean; message: string }
interface Props { settings: KdeIntegrationSettings; onChange: (settings: KdeIntegrationSettings) => Promise<void> }
export default function DecorationIntegrations({ settings, onChange }: Props) {
  const { t } = useTranslation();
  const [detection, setDetection] = useState<{ klassy: Detection; rounded_corners: Detection } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [opacity, setOpacity] = useState(settings.klassy.titlebar_opacity ?? 100);
  useEffect(() => setOpacity(settings.klassy.titlebar_opacity ?? 100), [settings]);
  const refresh = async () => {
    setBusy(true);
    try { setDetection(await invoke("detect_kde_integrations")); setError(null); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  useEffect(() => { void refresh(); }, []);
  const update = async (next: KdeIntegrationSettings) => {
    setBusy(true);
    try { await onChange(next); setError(null); } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  const status = (item: Detection | undefined) => !item ? t("advanced.detecting") : !item.detected ? t("advanced.notDetected") : !item.supported ? t("advanced.unsupported") : `${t("advanced.detected")}${item.version ? `: ${item.version}` : ""}${!item.active ? ` · ${t("advanced.enableInKde")}` : ""}`;
  return <section className="card desktop-card decoration-integrations">
    <div className="desktop-card-title"><h3>{t("advanced.integrations")}</h3><button type="button" className="btn btn-secondary btn-compact" disabled={busy} onClick={() => void refresh()} aria-label={t("desktop.refresh")}><RefreshCw size={14} /></button></div>
    <h4>Klassy</h4><p className="desktop-card-copy" role="status">{status(detection?.klassy)}</p>
    <fieldset disabled={busy || !detection?.klassy.supported}>
      <label className="desktop-toggle"><input type="checkbox" checked={settings.klassy.enabled} onChange={e => void update({ ...settings, klassy: { ...settings.klassy, enabled: e.target.checked } })} />{t("advanced.enableKlassy")}</label>
      <label className="desktop-toggle"><input type="checkbox" disabled={!settings.klassy.enabled} checked={settings.klassy.outline_sync} onChange={e => void update({ ...settings, klassy: { ...settings.klassy, outline_sync: e.target.checked } })} />{t("advanced.activeOutline")}</label>
      <label className="desktop-toggle"><input type="checkbox" disabled={!settings.klassy.enabled} checked={settings.klassy.titlebar_opacity !== null} onChange={e => void update({ ...settings, klassy: { ...settings.klassy, titlebar_opacity: e.target.checked ? opacity : null } })} />{t("advanced.manageOpacity")}</label>
      <label className="advanced-control"><span>{t("advanced.opacity")} <output>{opacity}%</output></span><input type="range" min={0} max={100} step={1} value={opacity} disabled={!settings.klassy.enabled || settings.klassy.titlebar_opacity === null} onChange={e => setOpacity(e.target.valueAsNumber)} /></label>
      <button type="button" className="btn btn-secondary btn-compact" disabled={!settings.klassy.enabled || settings.klassy.titlebar_opacity === null || settings.klassy.titlebar_opacity === opacity} onClick={() => void update({ ...settings, klassy: { ...settings.klassy, titlebar_opacity: opacity } })}>{t("advanced.apply")}</button>
    </fieldset>
    <h4>KDE Rounded Corners</h4><p className="desktop-card-copy" role="status">{status(detection?.rounded_corners)}</p>
    <fieldset disabled={busy || !detection?.rounded_corners.supported}>
      <label className="desktop-toggle"><input type="checkbox" checked={settings.rounded_corners.enabled} onChange={e => void update({ ...settings, rounded_corners: { ...settings.rounded_corners, enabled: e.target.checked } })} />{t("advanced.enableRounded")}</label>
      <label className="desktop-toggle"><input type="checkbox" disabled={!settings.rounded_corners.enabled} checked={settings.rounded_corners.outline_sync} onChange={e => void update({ ...settings, rounded_corners: { ...settings.rounded_corners, outline_sync: e.target.checked } })} />{t("advanced.syncOutline")}</label>
      <label className="advanced-control">{t("advanced.outlineSource")}<select disabled={!settings.rounded_corners.enabled || !settings.rounded_corners.outline_sync} value={settings.rounded_corners.outline_source} onChange={e => void update({ ...settings, rounded_corners: { ...settings.rounded_corners, outline_source: e.target.value as KdeIntegrationSettings["rounded_corners"]["outline_source"] } })}>
        <option value="automatic">{t("advanced.automatic")}</option><option value="primary">Primary</option><option value="outline">Outline</option><option value="outline_variant">Outline Variant</option>
      </select></label>
    </fieldset>
    {error && <p className="desktop-error" role="alert">{error}</p>}
  </section>;
}
