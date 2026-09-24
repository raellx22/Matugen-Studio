import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import Select from "../components/Select";
import { Switch } from "../components/Primitives";
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
    <div className="integration-groups">
      <div className="integration-group">
        <h4>Klassy</h4><p className="desktop-card-copy" role="status">{status(detection?.klassy)}</p>
        <fieldset disabled={busy || !detection?.klassy.supported}>
          <div className="desktop-toggle"><Switch label={t("advanced.enableKlassy")} checked={settings.klassy.enabled} disabled={busy || !detection?.klassy.supported} onChange={checked => void update({ ...settings, klassy: { ...settings.klassy, enabled: checked } })} /><span>{t("advanced.enableKlassy")}</span></div>
          <div className="desktop-toggle"><Switch label={t("advanced.activeOutline")} checked={settings.klassy.outline_sync} disabled={!settings.klassy.enabled || busy} onChange={checked => void update({ ...settings, klassy: { ...settings.klassy, outline_sync: checked } })} /><span>{t("advanced.activeOutline")}</span></div>
          <div className="desktop-toggle"><Switch label={t("advanced.manageOpacity")} checked={settings.klassy.titlebar_opacity !== null} disabled={!settings.klassy.enabled || busy} onChange={checked => void update({ ...settings, klassy: { ...settings.klassy, titlebar_opacity: checked ? opacity : null } })} /><span>{t("advanced.manageOpacity")}</span></div>
          <label className="advanced-control"><span>{t("advanced.opacity")} <output>{opacity}%</output></span><input type="range" min={0} max={100} step={1} value={opacity} disabled={!settings.klassy.enabled || settings.klassy.titlebar_opacity === null} onChange={e => setOpacity(e.target.valueAsNumber)} /></label>
          <button type="button" className="btn btn-secondary btn-compact" disabled={!settings.klassy.enabled || settings.klassy.titlebar_opacity === null || settings.klassy.titlebar_opacity === opacity} onClick={() => void update({ ...settings, klassy: { ...settings.klassy, titlebar_opacity: opacity } })}>{t("advanced.apply")}</button>
        </fieldset>
      </div>
      <div className="integration-group">
        <h4>KDE Rounded Corners</h4><p className="desktop-card-copy" role="status">{status(detection?.rounded_corners)}</p>
        <fieldset disabled={busy || !detection?.rounded_corners.supported}>
          <div className="desktop-toggle"><Switch label={t("advanced.enableRounded")} checked={settings.rounded_corners.enabled} disabled={busy || !detection?.rounded_corners.supported} onChange={checked => void update({ ...settings, rounded_corners: { ...settings.rounded_corners, enabled: checked } })} /><span>{t("advanced.enableRounded")}</span></div>
          <div className="desktop-toggle"><Switch label={t("advanced.syncOutline")} checked={settings.rounded_corners.outline_sync} disabled={!settings.rounded_corners.enabled || busy} onChange={checked => void update({ ...settings, rounded_corners: { ...settings.rounded_corners, outline_sync: checked } })} /><span>{t("advanced.syncOutline")}</span></div>
          <div className="advanced-control"><span>{t("advanced.outlineSource")}</span><Select label={t("advanced.outlineSource")} disabled={!settings.rounded_corners.enabled || !settings.rounded_corners.outline_sync || busy} value={settings.rounded_corners.outline_source}
            onValueChange={value => void update({ ...settings, rounded_corners: { ...settings.rounded_corners, outline_source: value as KdeIntegrationSettings["rounded_corners"]["outline_source"] } })}
            options={[{ value: "automatic", label: t("advanced.automatic") }, { value: "primary", label: "Primary" }, { value: "outline", label: "Outline" }, { value: "outline_variant", label: "Outline Variant" }]} /></div>
        </fieldset>
      </div>
    </div>
    {error && <p className="desktop-error" role="alert">{error}</p>}
  </section>;
}
