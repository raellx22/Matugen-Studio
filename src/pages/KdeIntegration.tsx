import { useEffect, useRef, useState } from "react";
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

interface KdeIntegrationProps {
  schemeData: any;
  wallpaperPath: string | null;
  schemeType: string;
  gtkThemeEnabled: boolean;
  gtkThemeDark: boolean;
  onGtkThemeEnabledChange: (enabled: boolean) => void;
  onGtkThemeDarkChange: (dark: boolean) => void;
  onGenerateFromWallpaper: (path: string) => Promise<any>;
}

interface KdeWatcherSnapshot {
  enabled: boolean;
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
  schemeData,
  schemeType,
  gtkThemeEnabled,
  gtkThemeDark,
  onGtkThemeEnabledChange,
  onGtkThemeDarkChange,
  onGenerateFromWallpaper,
}: KdeIntegrationProps) {
  const [serviceEnabled, setServiceEnabled] = useState(false);
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
      } else {
        const status = await invoke<KdeWatcherSnapshot>("stop_kde_wallpaper_watcher");
        applyWatcherSnapshot(status);
      }
    } catch (e) {
      alert("Failed to update service: " + e);
    }
  };

  const applyKdeScheme = async () => {
    if (!schemeData) {
      alert("Generate a color palette first from the Colors or Source tab.");
      return;
    }

    setIsApplyingScheme(true);
    setSchemeApplied(false);
    try {
      await invoke("apply_kde_colorscheme", { context: schemeData });
      setSchemeApplied(true);
      setStatusMessage("KDE color scheme applied successfully.");
      setTimeout(() => setSchemeApplied(false), 3000);
    } catch (e) {
      alert("Failed to apply KDE color scheme: " + e);
    } finally {
      setIsApplyingScheme(false);
    }
  };

  const generateAndApply = async () => {
    if (!currentKdeWallpaper) {
      alert("No wallpaper detected on KDE desktop.");
      return;
    }

    setIsApplyingScheme(true);
    try {
      const data = await onGenerateFromWallpaper(currentKdeWallpaper);
      setStatusMessage("Colors generated. Applying desktop themes...");
      const tasks: Promise<any>[] = [
        invoke("apply_theme", { context: data }),
        invoke("apply_kde_colorscheme", { context: data }),
      ];
      if (gtkThemeEnabled) {
        tasks.push(invoke("apply_gtk_theme", { context: data, dark: gtkThemeDark }));
      }
      const results = await Promise.allSettled(tasks);
      const failed = results.find((result) => result.status === "rejected");
      if (failed && failed.status === "rejected") throw failed.reason;
      setStatusMessage("Theme generated and applied from current wallpaper.");
      setSchemeApplied(true);
      setTimeout(() => setSchemeApplied(false), 3000);
    } catch (e) {
      alert("Failed to generate colors: " + e);
    } finally {
      setIsApplyingScheme(false);
    }
  };

  const applyGtkTheme = async (dark: boolean) => {
    if (!schemeData) {
      alert("Generate a color palette first from the Colors or Source tab.");
      return;
    }

    onGtkThemeDarkChange(dark);
    onGtkThemeEnabledChange(true);
    setIsApplyingGtk(true);
    try {
      const result = await invoke<GtkThemeResult>("apply_gtk_theme", { context: schemeData, dark });
      setGtkAppliedTheme(result.themeName);
      setStatusMessage(`GTK theme applied: ${result.themeName}`);
      const warningText = result.warnings.filter(Boolean).join("\n");
      if (warningText) alert(`GTK theme generated with warnings:\n${warningText}`);
      setTimeout(() => setGtkAppliedTheme(null), 3000);
    } catch (e) {
      alert("Failed to apply GTK theme: " + e);
    } finally {
      setIsApplyingGtk(false);
    }
  };

  return (
    <div className="desktop-page">
      <div className="desktop-header">
        <div>
          <h2>KDE Plasma Integration</h2>
          <p>Wallpaper monitoring, KDE colors, and GTK theme sync.</p>
        </div>
      </div>

      <div className="desktop-grid">
        <section className="card desktop-card">
          <div className="desktop-card-title">
            <Monitor size={20} />
            <h3>Current KDE Wallpaper</h3>
            <button className="btn btn-secondary btn-compact" onClick={fetchCurrentWallpaper}>
              <RefreshCw size={14} /> Refresh
            </button>
          </div>

          {currentKdeWallpaper ? (
            <div className="desktop-path">{currentKdeWallpaper}</div>
          ) : (
            <div className="desktop-warning">
              <AlertCircle size={14} />
              Could not detect current wallpaper
            </div>
          )}

          <button
            className="btn btn-primary"
            disabled={!currentKdeWallpaper || isApplyingScheme}
            onClick={generateAndApply}
          >
            {isApplyingScheme ? <Loader2 size={16} className="spinning" /> : <Palette size={16} />}
            Generate from Current Wallpaper
          </button>
        </section>

        <section className="card desktop-card">
          <div className="desktop-card-title">
            <Palette size={20} />
            <h3>KDE Color Scheme</h3>
          </div>
          <p className="desktop-card-copy">
            Generate and apply <code>MatugenStudio.colors</code> for Qt/KDE apps, window decorations, menus, and Plasma surfaces.
          </p>
          <button className="btn btn-primary" disabled={!schemeData || isApplyingScheme} onClick={applyKdeScheme}>
            {isApplyingScheme ? (
              <Loader2 size={16} className="spinning" />
            ) : schemeApplied ? (
              <CheckCircle2 size={16} />
            ) : (
              <Palette size={16} />
            )}
            {schemeApplied ? "Applied" : "Apply KDE Color Scheme"}
          </button>
        </section>

        <section className="card desktop-card">
          <div className="desktop-card-title">
            {serviceEnabled ? <Eye size={20} /> : <EyeOff size={20} />}
            <h3>Wallpaper Watch Service</h3>
          </div>
          <p className="desktop-card-copy">
            Backend watcher monitors KDE wallpaper changes and reapplies Matugen themes without a frontend timer.
          </p>
          <div className="desktop-inline-controls">
            <button className={`btn ${serviceEnabled ? "btn-secondary" : "btn-primary"}`} onClick={toggleService}>
              {serviceEnabled ? <Square size={16} /> : serviceProcessing ? <Loader2 size={16} className="spinning" /> : <Play size={16} />}
              {serviceEnabled ? "Stop Service" : "Start Service"}
            </button>
            <select value={pollInterval} onChange={(e) => setPollInterval(parseInt(e.target.value))}>
              <option value={5}>5 seconds</option>
              <option value={10}>10 seconds</option>
              <option value={30}>30 seconds</option>
              <option value={60}>1 minute</option>
            </select>
          </div>

          {statusMessage && (
            <div className={`desktop-status ${serviceError ? "is-error" : serviceEnabled ? "is-on" : ""}`}>
              {serviceEnabled && !serviceError && <span className="pulse-dot" />}
              {statusMessage}
            </div>
          )}
          {serviceError && <div className="desktop-error">{serviceError}</div>}
        </section>

        <section className="card desktop-card">
          <div className="desktop-card-title">
            <Palette size={20} />
            <h3>GTK Theme</h3>
          </div>
          <p className="desktop-card-copy">
            Generate a full adw-gtk3 based theme for GTK 3, GTK 4, and libadwaita apps.
          </p>

          <label className="desktop-toggle">
            <input
              type="checkbox"
              checked={gtkThemeEnabled}
              onChange={(event) => onGtkThemeEnabledChange(event.target.checked)}
            />
            <span>Apply GTK automatically when colors are generated</span>
          </label>

          <div className="desktop-segment">
            <button
              className={!gtkThemeDark ? "active" : ""}
              onClick={() => onGtkThemeDarkChange(false)}
              type="button"
            >
              Light
            </button>
            <button
              className={gtkThemeDark ? "active" : ""}
              onClick={() => onGtkThemeDarkChange(true)}
              type="button"
            >
              Dark
            </button>
          </div>

          <button
            className="btn btn-primary"
            disabled={!schemeData || isApplyingGtk}
            onClick={() => applyGtkTheme(gtkThemeDark)}
          >
            {isApplyingGtk ? <Loader2 size={16} className="spinning" /> : <Palette size={16} />}
            Apply GTK Theme
          </button>

          {gtkAppliedTheme && <div className="desktop-status is-on">{gtkAppliedTheme} applied.</div>}
        </section>
      </div>
    </div>
  );
}
