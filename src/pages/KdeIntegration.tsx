import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  AlertCircle,
  CheckCircle2,
  Eye,
  EyeOff,
  Loader2,
  Monitor,
  Palette,
  Play,
  RefreshCw,
  Square,
} from "lucide-react";

import DecorationIntegrations from "./DecorationIntegrations";
import Select from "../components/Select";
import { Switch } from "../components/Primitives";
import type { KdeIntegrationSettings } from "../utils/studioSettings";

type ThemeContext = Record<string, unknown>;

interface KdeIntegrationProps {
  integrationSettings: KdeIntegrationSettings;
  onIntegrationSettingsChange: (settings: KdeIntegrationSettings) => Promise<void>;
  schemeData: ThemeContext | null;
  themeContext: ThemeContext | null;
  wallpaperPath: string | null;
  schemeType: string;
  gtkThemeEnabled: boolean;
  gtkThemeDark: boolean;
  onGtkThemeEnabledChange: (enabled: boolean) => void;
  onGtkThemeDarkChange: (dark: boolean) => void;
  onGenerateFromWallpaper: (path: string) => Promise<{ raw: ThemeContext; kde: ThemeContext; effectiveDark: boolean }>;
  onApplyKdeColorscheme: (context: ThemeContext) => Promise<void>;
  onNotify?: (message: string, tone?: "success" | "error" | "info") => void;
}

interface KdeWatcherSnapshot {
  enabled: boolean;
  runInBackground: boolean;
  currentWallpaper: string | null;
  lastHandledWallpaper: string | null;
  isProcessing: boolean;
  pollIntervalSecs: number;
  schemeType: string;
  gtkEnabled: boolean;
  gtkDark: boolean;
  statusMessage: string;
  lastError: string | null;
}

interface GtkThemeResult {
  themeName: string;
  themePath: string;
  warnings: string[];
}

export default function KdeIntegration({
  integrationSettings,
  onIntegrationSettingsChange,
  schemeData,
  themeContext,
  schemeType,
  gtkThemeEnabled,
  gtkThemeDark,
  onGtkThemeEnabledChange,
  onGtkThemeDarkChange,
  onGenerateFromWallpaper,
  onApplyKdeColorscheme,
  onNotify,
}: KdeIntegrationProps) {
  const { t } = useTranslation();
  const [serviceEnabled, setServiceEnabled] = useState(false);
  const [runInBackground, setRunInBackground] = useState(false);
  const [currentKdeWallpaper, setCurrentKdeWallpaper] = useState<string | null>(null);
  const [isApplyingScheme, setIsApplyingScheme] = useState(false);
  const [schemeApplied, setSchemeApplied] = useState(false);
  const [pollInterval, setPollInterval] = useState(10);
  const [statusMessage, setStatusMessage] = useState("");
  const [serviceError, setServiceError] = useState<string | null>(null);
  const [serviceProcessing, setServiceProcessing] = useState(false);
  const [isApplyingGtk, setIsApplyingGtk] = useState(false);
  const [gtkAppliedTheme, setGtkAppliedTheme] = useState<string | null>(null);
  const didLoadStatusRef = useRef(false);

  useEffect(() => {
    loadServiceStatus();
    fetchCurrentWallpaper();
  }, []);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    listen<KdeWatcherSnapshot>("kde-wallpaper-watcher-status", (event) => {
      applyWatcherSnapshot(event.payload);
    }).then((dispose) => {
      unlisten = dispose;
    }).catch((e) => console.error("Failed to subscribe to KDE watcher:", e));

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    if (!didLoadStatusRef.current || !serviceEnabled) return;
    void startWatcher().catch((e) => {
      setServiceError(String(e));
      setStatusMessage("Failed to restart wallpaper watcher.");
    });
  }, [pollInterval, schemeType, gtkThemeEnabled, gtkThemeDark]);

  const startWatcher = async () => {
    const status = await invoke<KdeWatcherSnapshot>("start_kde_wallpaper_watcher", {
      options: {
        pollIntervalSecs: pollInterval,
        schemeType,
        gtkEnabled: gtkThemeEnabled,
        gtkDark: gtkThemeDark,
      },
    });
    applyWatcherSnapshot(status);
  };

  const loadServiceStatus = async () => {
    try {
      const status = await invoke<KdeWatcherSnapshot>("get_kde_wallpaper_watcher_status");
      applyWatcherSnapshot(status);
      if (status.gtkEnabled) onGtkThemeEnabledChange(true);
      onGtkThemeDarkChange(status.gtkDark);
      didLoadStatusRef.current = true;
    } catch (e) {
      console.error("Failed to load service status:", e);
    }
  };

  const applyWatcherSnapshot = (snapshot: KdeWatcherSnapshot) => {
    setServiceEnabled(snapshot.enabled);
    setRunInBackground(snapshot.runInBackground);
    setCurrentKdeWallpaper(snapshot.currentWallpaper);
    setServiceProcessing(snapshot.isProcessing);
    setPollInterval(snapshot.pollIntervalSecs);
    setStatusMessage(snapshot.statusMessage);
    setServiceError(snapshot.lastError);
  };

  const fetchCurrentWallpaper = async () => {
    try {
      const wp: string | null = await invoke("get_kde_current_wallpaper");
      setCurrentKdeWallpaper(wp);
    } catch (e) {
      console.error("Failed to get current wallpaper:", e);
    }
  };

  const toggleService = async () => {
    setServiceError(null);
    try {
      if (!serviceEnabled) {
        await startWatcher();
        onNotify?.("Wallpaper watch service started.", "success");
      } else {
        const status = await invoke<KdeWatcherSnapshot>("stop_kde_wallpaper_watcher");
        applyWatcherSnapshot(status);
        onNotify?.("Wallpaper watch service stopped.", "info");
      }
    } catch (e) {
      onNotify?.("Failed to update service: " + e, "error");
    }
  };

  const toggleRunInBackground = async (enabled: boolean) => {
    setServiceError(null);
    try {
      const status = await invoke<KdeWatcherSnapshot>("set_kde_run_in_background", { enabled });
      applyWatcherSnapshot(status);
      onNotify?.(
        enabled ? t('desktop.backgroundEnabledToast') : t('desktop.backgroundDisabledToast'),
        "info",
      );
    } catch (e) {
      onNotify?.("Failed to update background option: " + e, "error");
    }
  };

  const applyKdeScheme = async () => {
    if (!schemeData) {
      onNotify?.("Generate a color palette first from the Colors or Source tab.", "error");
      return;
    }

    setIsApplyingScheme(true);
    setSchemeApplied(false);
    try {
      await onApplyKdeColorscheme(schemeData);
      setSchemeApplied(true);
      setStatusMessage("KDE color scheme applied successfully.");
      onNotify?.("KDE color scheme applied.", "success");
      setTimeout(() => setSchemeApplied(false), 3000);
    } catch (e) {
      onNotify?.("Failed to apply KDE color scheme: " + e, "error");
    } finally {
      setIsApplyingScheme(false);
    }
  };

  const generateAndApply = async () => {
    if (!currentKdeWallpaper) {
      onNotify?.("No wallpaper detected on KDE desktop.", "error");
      return;
    }

    setIsApplyingScheme(true);
    try {
      const { raw, kde, effectiveDark } = await onGenerateFromWallpaper(currentKdeWallpaper);
      setStatusMessage("Colors generated. Applying desktop themes...");
      const tasks: Promise<unknown>[] = [
        invoke("apply_theme", { context: raw }),
        onApplyKdeColorscheme(kde),
      ];
      if (gtkThemeEnabled) {
        onGtkThemeDarkChange(effectiveDark);
        tasks.push(invoke("apply_gtk_theme", { context: raw, dark: effectiveDark }));
      }
      const results = await Promise.allSettled(tasks);
      const failed = results.find((result) => result.status === "rejected");
      if (failed && failed.status === "rejected") throw failed.reason;
      setStatusMessage("Theme generated and applied from current wallpaper.");
      setSchemeApplied(true);
      onNotify?.("Theme generated from current KDE wallpaper.", "success");
      setTimeout(() => setSchemeApplied(false), 3000);
    } catch (e) {
      onNotify?.("Failed to generate colors: " + e, "error");
    } finally {
      setIsApplyingScheme(false);
    }
  };

  const applyGtkTheme = async (dark: boolean) => {
    if (!themeContext) {
      onNotify?.("Generate a color palette first from the Colors or Source tab.", "error");
      return;
    }

    onGtkThemeDarkChange(dark);
    onGtkThemeEnabledChange(true);
    setIsApplyingGtk(true);
    try {
      const result = await invoke<GtkThemeResult>("apply_gtk_theme", { context: themeContext, dark });
      setGtkAppliedTheme(result.themeName);
      setStatusMessage(`GTK theme applied: ${result.themeName}`);
      const warningText = result.warnings.filter(Boolean).join("\n");
      if (warningText) {
        onNotify?.(`GTK theme generated with warnings: ${warningText}`, "info");
      } else {
        onNotify?.(`GTK theme applied: ${result.themeName}`, "success");
      }
      setTimeout(() => setGtkAppliedTheme(null), 3000);
    } catch (e) {
      onNotify?.("Failed to apply GTK theme: " + e, "error");
    } finally {
      setIsApplyingGtk(false);
    }
  };

  return (
    <div className="desktop-page">
      <div className="desktop-header">
        <div>
          <h2>{t('desktop.title')}</h2>
          <p>{t('desktop.subtitle')}</p>
        </div>
      </div>

      <div className="desktop-grid">
        <div className="desktop-domain">
          <DecorationIntegrations settings={integrationSettings} onChange={onIntegrationSettingsChange} />
        </div>
        <div className="desktop-domain"><h3>{t('desktop.kdeColorScheme')} / {t('desktop.gtkTheme')}</h3>
<section className="card desktop-card">
          <div className="desktop-card-title">
            <Palette size={20} />
            <h3>{t('desktop.kdeColorScheme')}</h3>
          </div>
          <p className="desktop-card-copy">
            {t('desktop.kdeColorDesc')}
          </p>
          <button className="btn btn-primary" disabled={!schemeData || isApplyingScheme} onClick={applyKdeScheme}>
            {isApplyingScheme ? (
              <Loader2 size={16} className="spinning" />
            ) : schemeApplied ? (
              <CheckCircle2 size={16} />
            ) : (
              <Palette size={16} />
            )}
            {schemeApplied ? t('desktop.applied') : t('desktop.applyKdeScheme')}
          </button>
        </section>
<section className="card desktop-card">
          <div className="desktop-card-title">
            <Palette size={20} />
            <h3>{t('desktop.gtkTheme')}</h3>
          </div>
          <p className="desktop-card-copy">
            {t('desktop.gtkDesc')}
          </p>

          <div className="desktop-toggle"><Switch label={t('desktop.applyGtkAuto')} checked={gtkThemeEnabled} onChange={onGtkThemeEnabledChange} /><span>{t('desktop.applyGtkAuto')}</span></div>

          <div className="desktop-segment">
            <button
              className={!gtkThemeDark ? "active" : ""}
              onClick={() => onGtkThemeDarkChange(false)}
              type="button"
            >
              {t('desktop.light')}
            </button>
            <button
              className={gtkThemeDark ? "active" : ""}
              onClick={() => onGtkThemeDarkChange(true)}
              type="button"
            >
              {t('desktop.dark')}
            </button>
          </div>

          <button
            className="btn btn-primary"
            disabled={!schemeData || isApplyingGtk}
            onClick={() => applyGtkTheme(gtkThemeDark)}
          >
            {isApplyingGtk ? <Loader2 size={16} className="spinning" /> : <Palette size={16} />}
            {t('desktop.applyGtkTheme')}
          </button>

          {gtkAppliedTheme && <div className="desktop-status is-on">{t('desktop.gtkApplied', { name: gtkAppliedTheme })}</div>}
        </section>
        </div>
        <div className="desktop-domain"><h3>{t('desktop.currentKdeWallpaper')} / {t('desktop.watcherService')}</h3>
<section className="card desktop-card">
          <div className="desktop-card-title">
            <Monitor size={20} />
            <h3>{t('desktop.currentKdeWallpaper')}</h3>
            <button className="btn btn-secondary btn-compact" onClick={fetchCurrentWallpaper}>
              <RefreshCw size={14} /> {t('desktop.refresh')}
            </button>
          </div>

          {currentKdeWallpaper ? (
            <div className="desktop-path">{currentKdeWallpaper}</div>
          ) : (
            <div className="desktop-warning">
              <AlertCircle size={14} />
              {t('desktop.noWallpaperDetected')}
            </div>
          )}

          <button
            className="btn btn-primary"
            disabled={!currentKdeWallpaper || isApplyingScheme}
            onClick={generateAndApply}
          >
            {isApplyingScheme ? <Loader2 size={16} className="spinning" /> : <Palette size={16} />}
            {t('desktop.generateFromCurrent')}
          </button>
        </section>
<section className="card desktop-card">
          <div className="desktop-card-title">
            {serviceEnabled ? <Eye size={20} /> : <EyeOff size={20} />}
            <h3>{t('desktop.watcherService')}</h3>
          </div>
          <p className="desktop-card-copy">
            {t('desktop.watcherDesc')}
          </p>
          <div className="desktop-inline-controls">
            <button className={`btn ${serviceEnabled ? "btn-secondary" : "btn-primary"}`} onClick={toggleService}>
              {serviceEnabled ? <Square size={16} /> : serviceProcessing ? <Loader2 size={16} className="spinning" /> : <Play size={16} />}
              {serviceEnabled ? t('desktop.stopService') : t('desktop.startService')}
            </button>
            <Select label={t('desktop.watcherService')} value={String(pollInterval)} onValueChange={value => setPollInterval(Number(value))}
              options={[5, 10, 30, 60].map((value, index) => ({ value: String(value), label: t(['desktop.seconds_5','desktop.seconds_10','desktop.seconds_30','desktop.minute_1'][index]) }))} />
          </div>

          <div className="desktop-toggle"><Switch label={t('desktop.runInBackground')} checked={runInBackground} onChange={value => void toggleRunInBackground(value)} /><span>{t('desktop.runInBackground')}</span></div>
          <p className="desktop-card-copy">
            {t('desktop.runInBackgroundDesc')}
          </p>

          {statusMessage && (
            <div className={`desktop-status ${serviceError ? "is-error" : serviceEnabled ? "is-on" : ""}`}>
              {serviceEnabled && !serviceError && <span className="pulse-dot" />}
              {statusMessage}
            </div>
          )}
          {serviceError && <div className="desktop-error">{serviceError}</div>}
        </section>
        </div>
      </div>
    </div>
  );
}
