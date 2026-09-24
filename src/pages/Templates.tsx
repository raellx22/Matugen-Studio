import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { confirm, message } from "@tauri-apps/plugin-dialog";
import { LayoutTemplate, Search, X } from "lucide-react";
import Select from "../components/Select";
import { PageHeader } from "../components/Primitives";

interface TemplateInfo {
  name: string;
  displayName: string;
  path: string;
  relativePath: string;
  size: number;
  category: string;
  targetApp: string;
  automationLevel: string;
  installable: boolean;
  defaultOutputPath?: string;
  defaultPostHook?: string;
  requiredCommands: string[];
  manualSteps: string[];
  relatedFiles: string[];
}

interface CatalogTarget {
  id: string;
  label: string;
  installType: "native" | "flatpak" | "manual";
  inputPath: string;
  outputPath: string;
  detected: boolean;
}

interface CatalogVariant {
  id: string;
  name: string;
  description?: string;
  source: {
    kind: "official" | "community";
    name: string;
    repository?: string;
    author?: string;
    license?: string;
    licenseStatus?: string;
    attribution?: string;
    sourcePath?: string;
    upstreamCommit?: string;
    version?: string;
  };
  targets: CatalogTarget[];
  template: TemplateInfo;
}

interface CatalogApp {
  id: string;
  name: string;
  category: string;
  variants: CatalogVariant[];
}

interface InstalledTemplateEntry {
  key: string;
  inputPath: string;
  outputPath: string;
}

interface TemplatesProps {
  schemeData: Record<string, unknown> | null;
}

const pairKey = (variant: CatalogVariant, target: CatalogTarget) => `${variant.id}__${target.id}`;
const variantDescriptionKeys: Record<string, string> = {
  "discord.material": "templates.variantDescriptions.discordMaterial",
  "vscode.material-premium": "templates.variantDescriptions.vscodeMaterialPremium",
};

function colorizeText(text: string) {
  const colorRegex = /(#[A-Fa-f0-9]{8}|#[A-Fa-f0-9]{6}|#[A-Fa-f0-9]{3}|rgba?\([^)]+\))/g;
  return text.split(colorRegex).map((part, index) => {
    if (!part.match(colorRegex)) return <span key={index}>{part}</span>;
    let isLight = false;
    if (part.startsWith("#")) {
      let hex = part.slice(1);
      if (hex.length === 3) hex = hex.split("").map(char => char + char).join("");
      const red = parseInt(hex.slice(0, 2), 16);
      const green = parseInt(hex.slice(2, 4), 16);
      const blue = parseInt(hex.slice(4, 6), 16);
      isLight = 0.2126 * red + 0.7152 * green + 0.0722 * blue > 128;
    }
    return <span key={index} className="template-color" style={{ backgroundColor: part, color: isLight ? "#000" : "#fff" }}>{part}</span>;
  });
}

export default function Templates({ schemeData }: TemplatesProps) {
  const { t } = useTranslation();
  const [catalog, setCatalog] = useState<CatalogApp[]>([]);
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [installedEntries, setInstalledEntries] = useState<InstalledTemplateEntry[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [category, setCategory] = useState("all");
  const [source, setSource] = useState("all");
  const [selectedApp, setSelectedApp] = useState<CatalogApp | null>(null);
  const [variantId, setVariantId] = useState("");
  const [targetId, setTargetId] = useState("");
  const [outputPath, setOutputPath] = useState("");
  const [postHook, setPostHook] = useState("");
  const [isInstalling, setIsInstalling] = useState(false);
  const [previewContent, setPreviewContent] = useState<string | null>(null);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);
  const previewRequest = useRef(0);
  const detailDialog = useRef<HTMLDivElement>(null);
  const returnFocus = useRef<HTMLElement | null>(null);

  const fetchInstalled = async () => {
    try {
      const [keys, entries] = await Promise.all([
        invoke<string[]>("get_installed_templates"),
        invoke<InstalledTemplateEntry[]>("get_installed_template_entries")
      ]);
      setInstalled(new Set(keys));
      setInstalledEntries(entries);
    } catch (error) {
      console.error("Failed to fetch installed templates", error);
    }
  };

  useEffect(() => {
    invoke<CatalogApp[]>("list_template_catalog")
      .then(setCatalog)
      .catch(error => console.error("Failed to load template catalog", error))
      .finally(() => setIsLoading(false));
    fetchInstalled();
  }, []);

  const categoryLabel = (value: string) => t(`templates.categories.${value}`, { defaultValue: value });
  const variantsFor = (app: CatalogApp) => app.variants.filter(variant =>
    source === "all" || variant.source.kind === source
  );
  const categories = ["all", ...new Set(catalog.map(app => app.category))];
  const filtered = catalog.filter(app => {
    const variants = variantsFor(app);
    const query = search.trim().toLocaleLowerCase();
    return variants.length > 0 && (category === "all" || app.category === category) &&
      (!query || [app.name, app.category, categoryLabel(app.category), ...variants.flatMap(variant => [variant.name, variant.description ?? ""])]
        .some(value => value.toLocaleLowerCase().includes(query)));
  });
  const selectedVariants = selectedApp ? variantsFor(selectedApp) : [];
  const variant = selectedVariants.find(item => item.id === variantId);
  const target = variant?.targets.find(item => item.id === targetId);

  const installedName = (item: CatalogVariant, destination: CatalogTarget) => {
    const key = pairKey(item, destination);
    if (installed.has(key)) return key;
    const sourceName = destination.inputPath.split(/[\\/]/).pop() ?? "";
    const templateName = item.template.name.split(/[\\/]/).pop() ?? "";
    const legacyKeys = [sourceName, templateName].map(name => name.replace(/[^A-Za-z0-9]/g, "_"));
    const legacyDiscordOutput = (item.id === "discord-midnight" || item.id === "discord-system24") ?
      `${destination.outputPath.slice(0, destination.outputPath.lastIndexOf("/") + 1)}matugen.css` : null;
    const legacy = installedEntries.find(entry =>
      installed.has(entry.key) && legacyKeys.includes(entry.key) &&
      (entry.outputPath === destination.outputPath || entry.outputPath === legacyDiscordOutput) &&
      entry.inputPath.split(/[\\/]/).pop()?.replace(/[^A-Za-z0-9]/g, "_") === entry.key
    );
    return legacy?.inputPath.split(/[\\/]/).pop() ?? null;
  };

  const resetPreview = () => {
    previewRequest.current++;
    setPreviewContent(null);
    setIsPreviewLoading(false);
  };

  const closeDetail = () => {
    if (isInstalling) return;
    resetPreview();
    setSelectedApp(null);
    returnFocus.current?.focus();
  };

  useEffect(() => {
    if (!selectedApp) return;
    detailDialog.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (event.key === "Escape") {
        event.preventDefault();
        closeDetail();
      } else if (event.key === "Tab") {
        const focusable = detailDialog.current?.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input:not(:disabled), a[href]'
        );
        if (!focusable?.length) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (event.shiftKey && (document.activeElement === first || document.activeElement === detailDialog.current)) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [selectedApp, isInstalling]);

  const chooseVariant = (item: CatalogVariant) => {
    resetPreview();
    const destination = item.targets.find(candidate => candidate.detected) ?? item.targets[0];
    setVariantId(item.id);
    setTargetId(destination?.id ?? "");
    setOutputPath(destination?.outputPath ?? item.template.defaultOutputPath ?? "");
    setPostHook(item.template.defaultPostHook ?? "");
  };

  const openDetail = (app: CatalogApp, trigger: HTMLElement) => {
    const variants = variantsFor(app);
    if (!variants.length) return;
    returnFocus.current = trigger;
    const query = search.trim().toLocaleLowerCase();
    chooseVariant(variants.find(item => query && [item.name, item.description ?? ""]
      .some(value => value.toLocaleLowerCase().includes(query))) ?? variants[0]);
    setSelectedApp(app);
  };

  const chooseTarget = (id: string) => {
    const destination = variant?.targets.find(item => item.id === id);
    if (!destination) return;
    resetPreview();
    setTargetId(id);
    setOutputPath(destination.outputPath);
  };

  const preview = async () => {
    if (!variant) return;
    if (!schemeData) {
      await message(t("templates.previewError"), { title: t("templates.preview"), kind: "warning" });
      return;
    }
    const request = ++previewRequest.current;
    setPreviewContent(null);
    setIsPreviewLoading(true);
    try {
      const result = await invoke<string>("preview_template", { templatePath: target?.inputPath ?? variant.template.path, context: schemeData });
      if (request === previewRequest.current) setPreviewContent(result);
    } catch (error) {
      if (request === previewRequest.current) {
        await message(t("templates.previewFailed", { error: String(error) }), { title: t("templates.preview"), kind: "error" });
      }
    } finally {
      if (request === previewRequest.current) setIsPreviewLoading(false);
    }
  };

  const install = async () => {
    if (!variant || !target || !variant.template.installable) return;
    const path = outputPath.trim();
    if (!path) {
      await message(t("templates.outputPathRequired"), { title: t("templates.installTemplate"), kind: "warning" });
      return;
    }
    const approved = await confirm(t("templates.installConfirm", { name: variant.name, target: target.label, path }), {
      title: t("templates.installTemplate"), kind: "warning"
    });
    if (!approved) return;
    setIsInstalling(true);
    try {
      await invoke("install_template", {
        templatePath: target.inputPath,
        templateName: pairKey(variant, target),
        outputPath: path,
        postHook
      });
      await fetchInstalled();
      await message(t("templates.installedSuccess", { name: variant.name }), { title: t("templates.installTemplate"), kind: "info" });
    } catch (error) {
      await message(t("templates.installError", { error: String(error) }), { title: t("templates.installTemplate"), kind: "error" });
    } finally {
      setIsInstalling(false);
    }
  };

  const uninstall = async () => {
    if (!variant || !target) return;
    const name = installedName(variant, target);
    if (!name) return;
    const approved = await confirm(t("templates.removeConfirm", { name: `${variant.name} — ${target.label}` }), {
      title: t("templates.remove"), kind: "warning"
    });
    if (!approved) return;
    setIsInstalling(true);
    try {
      await invoke("uninstall_template", { templateName: name });
      await fetchInstalled();
    } catch (error) {
      await message(t("templates.uninstallError", { error: String(error) }), { title: t("templates.remove"), kind: "error" });
    } finally {
      setIsInstalling(false);
    }
  };

  const automationLabel = (level: string) => level === "auto" ? t("templates.automation.auto") :
    level === "config-patch" ? t("templates.automation.config") : t("templates.automation.manual");

  return <div className="templates-page page-transition">
    <PageHeader title={t("templates.title")} description={t("templates.subtitle")} />
    <div className="template-filters">
      <div className="template-search">
        <Search size={18} aria-hidden="true" />
        <input type="search" aria-label={t("templates.searchPlaceholder")} placeholder={t("templates.searchPlaceholder")} value={search} onChange={event => setSearch(event.target.value)} />
      </div>
      <Select label={t("templates.category")} value={category} onValueChange={setCategory}
        options={categories.map(value => ({ value, label: value === "all" ? t("templates.allCategories") : categoryLabel(value) }))} />
      <Select label={t("templates.sourceFilter")} value={source} onValueChange={setSource}
        options={["all", "official", "community"].map(value => ({ value, label: t(`templates.sources.${value}`) }))} />
    </div>

    <div className="collection-grid">
      {isLoading ? <p>{t("templates.loading")}</p> : filtered.length === 0 ?
        <div className="collection-empty"><LayoutTemplate size={28} aria-hidden="true" />
          <strong>{t("templates.noTemplates")}</strong>
          {source === "community" && !catalog.some(app =>
            (category === "all" || app.category === category) && app.variants.some(item => item.source.kind === "community")
          ) && <p>{t("templates.communityUnavailable")}</p>}
        </div> : filtered.map(app => {
          const variants = variantsFor(app);
          const installedCount = variants.reduce((count, item) => count + item.targets.filter(destination => !!installedName(item, destination)).length, 0);
          return <article key={app.id} className="template-card app-template-card">
            <div className="template-card-heading"><span className="template-card-icon"><LayoutTemplate size={24} aria-hidden="true" /></span>
              <div><h3>{app.name}</h3><span>{categoryLabel(app.category)}</span></div>
            </div>
            <div className="template-card-summary">
              <span className="template-badge">{t("templates.variantCount", { count: variants.length })}</span>
              <span className="template-badge">{t("templates.installedCount", { count: installedCount })}</span>
            </div>
            <p className="template-card-variants">{variants.map(item => item.name).join(" · ")}{variants.some(item => item.source.kind === "community") && <> · <span className="template-card-community">{t("templates.sources.community")}</span></>}</p>
            <button type="button" className="btn btn-secondary btn-compact template-card-action"
              onClick={event => openDetail(app, event.currentTarget)}>{t("templates.details")}</button>
          </article>;
        })}
    </div>

    {selectedApp && variant && <div className="modal-overlay" onClick={closeDetail}>
      <div className="modal-content templates-dialog" role="dialog" aria-modal="true" aria-label={selectedApp.name}
        ref={detailDialog} tabIndex={-1} onClick={event => event.stopPropagation()}>
        <div className="modal-header"><div><h3>{selectedApp.name}</h3><span className="template-badge">{categoryLabel(selectedApp.category)}</span></div>
          <button type="button" className="btn btn-ghost icon-btn" aria-label={t("templates.close")} onClick={closeDetail}><X size={20} /></button>
        </div>
        <div className="modal-body template-detail-body">
          <div className="template-detail-fields">
            <label>{t("templates.variant")}
              <Select label={t("templates.variant")} value={variant.id} onValueChange={id => {
                const next = selectedVariants.find(item => item.id === id);
                if (next) chooseVariant(next);
              }} options={selectedVariants.map(item => ({ value: item.id, label: item.name }))} />
            </label>
            {variant.description && <p>{variantDescriptionKeys[variant.id] ?
              t(variantDescriptionKeys[variant.id], { defaultValue: variant.description }) : variant.description}</p>}
            <div className="template-card-summary">
              <span className={`template-badge automation-${variant.template.automationLevel}`}>{automationLabel(variant.template.automationLevel)}</span>
              {!variant.template.installable && <span className="template-badge">{t("templates.asset")}</span>}
              {variant.template.requiredCommands.map(command => <span key={command} className="template-badge">{t("templates.needs", { cmd: command })}</span>)}
            </div>
          </div>
          <section className="template-source" aria-label={t("templates.sourceDetails")}>
            <strong>{t("templates.sourceDetails")}: {variant.source.name}</strong>
            <span>{t(`templates.sources.${variant.source.kind}`)}{variant.source.author ? ` · ${variant.source.author}` : ""}</span>
            {variant.source.license && <span>{t("templates.license")}: {variant.source.license}</span>}
            {variant.source.licenseStatus && <span>{t("templates.licenseStatus")}: {t(`templates.licenseStatuses.${variant.source.licenseStatus}`, { defaultValue: variant.source.licenseStatus })}</span>}
            {variant.source.attribution && <span>{t(`templates.variantAttributions.${variant.id.replace(/[^A-Za-z0-9]/g, "_")}`, { defaultValue: variant.source.attribution })}</span>}
            {variant.source.version && <span>{t("templates.version")}: {variant.source.version}</span>}
            {variant.source.upstreamCommit && <span>{t("templates.commit")}: {variant.source.upstreamCommit}</span>}
            {variant.source.sourcePath && <span className="template-path">{variant.source.sourcePath}</span>}
            {variant.source.repository && <a href={variant.source.repository} target="_blank" rel="noopener noreferrer">{t("templates.repository")}</a>}
          </section>
          <div className="template-detail-fields">
            <label>{t("templates.target")}
              <Select label={t("templates.target")} value={targetId} onValueChange={chooseTarget}
                options={variant.targets.map(item => ({ value: item.id, label: item.label }))} />
            </label>
            {target && <>
              <span className={`template-detection ${target.detected ? "is-detected" : ""}`}>
                {target.detected ? t("templates.detected") : t("templates.notDetected")}
                {` · ${t(`templates.installTypes.${target.installType}`)}`}
              </span>
              <label htmlFor="template-output-path">{t("templates.outputPath")}</label>
              <input id="template-output-path" type="text" value={outputPath} onChange={event => setOutputPath(event.target.value)}
                aria-invalid={!outputPath.trim()} placeholder={target.outputPath} />
              {!outputPath.trim() && <span className="template-field-error">{t("templates.outputPathRequired")}</span>}
            </>}
            <label htmlFor="template-post-hook">{t("templates.postHook")}</label>
            <input id="template-post-hook" type="text" value={postHook} onChange={event => setPostHook(event.target.value)} />
          </div>
          {variant.template.manualSteps.length > 0 && <section className="template-manual-steps">
            <strong>{t("templates.manualStepsNeeded")}</strong>
            <ol>{variant.template.manualSteps.map((step, index) => <li key={index}>{t(step, { defaultValue: step })}</li>)}</ol>
          </section>}
          {(isPreviewLoading || previewContent !== null) && <section className="template-preview" aria-label={t("templates.previewTitle", { name: variant.name })}>
            {isPreviewLoading ? <p>{t("templates.rendering")}</p> : <pre>{colorizeText(previewContent ?? "")}</pre>}
          </section>}
        </div>
        <div className="modal-actions template-detail-actions">
          <button type="button" className="btn btn-secondary" onClick={closeDetail} disabled={isInstalling}>{t("templates.close")}</button>
          <button type="button" className="btn btn-secondary" onClick={preview} disabled={isPreviewLoading}>{t("templates.preview")}</button>
          {target && (installedName(variant, target) ?
            <button type="button" className="btn btn-danger" onClick={uninstall} disabled={isInstalling}>{t("templates.remove")}</button> :
            <button type="button" className="btn btn-primary" onClick={install} disabled={isInstalling || !variant.template.installable || !outputPath.trim()}>
              {isInstalling ? t("templates.installing") : variant.template.installable ? t("templates.install") : t("templates.guide")}
            </button>)}
        </div>
      </div>
    </div>}
  </div>;
}
