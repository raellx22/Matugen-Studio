import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { Search, LayoutTemplate, X } from "lucide-react";

interface TemplateInfo {
  name: string;
  displayName: string;
  path: string;
  relativePath: string;
  size: number;
  category: string;
  targetApp: string;
  automationLevel: "auto" | "config-patch" | "manual" | string;
  installable: boolean;
  defaultOutputPath?: string;
  defaultPostHook?: string;
  requiredCommands: string[];
  manualSteps: string[];
  relatedFiles: string[];
}

interface TemplatesProps {
  schemeData: Record<string, unknown> | null;
}

interface DiscordClient {
  name: string;
  path: string;
  exists: boolean;
}

export default function Templates({ schemeData }: TemplatesProps) {
  const { t } = useTranslation();
  const [templates, setTemplates] = useState<TemplateInfo[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [activeCategory, setActiveCategory] = useState("All");
  const [previewContent, setPreviewContent] = useState<string | null>(null);

  const [previewName, setPreviewName] = useState<string | null>(null);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);
  const [installTemplate, setInstallTemplate] = useState<TemplateInfo | null>(null);
  const [outputPath, setOutputPath] = useState("");
  const [postHook, setPostHook] = useState("");
  const [isInstalling, setIsInstalling] = useState(false);
  const [discordClients, setDiscordClients] = useState<DiscordClient[]>([]);

  // We add an effect to check Discord clients when the modal opens for midnight-discord.css
  useEffect(() => {
    if (installTemplate && installTemplate.relativePath.endsWith("midnight-discord.css")) {
      checkDiscordClients();
    }
  }, [installTemplate]);

  const checkDiscordClients = async () => {
    const clients = [
      { name: "Vesktop", path: "~/.config/vesktop" },
      { name: "Vesktop (Flatpak)", path: "~/.var/app/dev.vencord.Vesktop/config/vesktop" },
      { name: "Equibop", path: "~/.config/equibop" },
      { name: "Equibop (Flatpak)", path: "~/.var/app/io.github.equicord.equibop/config/equibop" },
      { name: "Vencord", path: "~/.config/Vencord" },
      { name: "Discord (Flatpak)", path: "~/.var/app/com.discordapp.Discord/config/discord" },
    ];
    
    const results = await Promise.all(
      clients.map(async (client) => {
        try {
          const exists = await invoke<boolean>("dir_exists", { path: client.path });
          return { ...client, exists };
        } catch (e) {
          return { ...client, exists: false };
        }
      })
    );
    setDiscordClients(results);
  };

  const [installedTemplates, setInstalledTemplates] = useState<Set<string>>(new Set());

  const fetchInstalled = async () => {
    try {
      const installed: string[] = await invoke("get_installed_templates");
      setInstalledTemplates(new Set(installed));
    } catch (e) {
      console.error("Failed to fetch installed templates", e);
    }
  };

  useEffect(() => {
    async function fetchTemplates() {
      try {
        const data: TemplateInfo[] = await invoke("list_bundled_templates");
        setTemplates(data);
      } catch (err) {
        console.error("Error fetching templates", err);
      } finally {
        setIsLoading(false);
      }
    }
    fetchTemplates();
    fetchInstalled();
  }, []);

  const categories = ["All", ...Array.from(new Set(templates.map(t => t.category)))];

  const filteredTemplates = templates.filter(t => {
    const q = searchQuery.toLowerCase();
    const matchesSearch =
      t.displayName.toLowerCase().includes(q) ||
      t.targetApp.toLowerCase().includes(q) ||
      t.relativePath.toLowerCase().includes(q);
    const matchesCategory = activeCategory === "All" || t.category === activeCategory;
    return matchesSearch && matchesCategory;
  });

  const installedKey = (templateName: string) => {
    return templateName.replace(/[^A-Za-z0-9]/g, "_");
  };

  const automationLabel = (level: string) => {
    if (level === "auto") return t('templates.automation.auto');
    if (level === "config-patch") return t('templates.automation.config');
    return t('templates.automation.manual');
  };

  const automationColor = (level: string) => {
    if (level === "auto") return "#8bd5a9";
    if (level === "config-patch") return "#f5d37a";
    return "#f2a7a7";
  };

  const handlePreview = async (template: TemplateInfo) => {
    if (!schemeData) {
      alert("Please extract colors from an image first in the Colors tab!");
      return;
    }
    
    setIsPreviewLoading(true);
    setPreviewName(template.displayName);
    setPreviewContent(null);
    
    try {
      const result: string = await invoke("preview_template", {
        templatePath: template.path,
        context: schemeData
      });
      setPreviewContent(result);
    } catch (err: any) {
      console.error(err);
      setPreviewContent(`Error generating preview:\n${err}`);
    } finally {
      setIsPreviewLoading(false);
    }
  };

  const handleInstallClick = (template: TemplateInfo) => {
    setInstallTemplate(template);
    setOutputPath(template.defaultOutputPath || "");
    setPostHook(template.defaultPostHook || "");
  };

  const submitInstall = async () => {
    if (!installTemplate) return;
    setIsInstalling(true);
    try {
      await invoke("install_template", {
        templatePath: installTemplate.path,
        templateName: installTemplate.name,
        outputPath: outputPath,
        postHook: postHook
      });
      alert(`Template ${installTemplate.displayName} installed successfully!`);
      setInstallTemplate(null);
      fetchInstalled();
    } catch (err: any) {
      alert(`Error installing template:\n${err}`);
    } finally {
      setIsInstalling(false);
    }
  };

  const handleUninstall = async (template: TemplateInfo) => {
    if (confirm(t('templates.removeConfirm', { name: template.displayName }))) {
      try {
        await invoke("uninstall_template", { templateName: template.name });
        fetchInstalled();
      } catch (e: any) {
        alert("Failed to uninstall: " + e);
      }
    }
  };

  const colorizeText = (text: string) => {
    if (!text) return null;
    const colorRegex = /(#[A-Fa-f0-9]{8}|#[A-Fa-f0-9]{6}|#[A-Fa-f0-9]{3}|rgba?\([^)]+\))/g;
    const parts = text.split(colorRegex);

    return parts.map((part, i) => {
      if (part.match(colorRegex)) {
        let isLight = false;
        if (part.startsWith('#')) {
          let hex = part.replace('#', '');
          if (hex.length === 3) hex = hex.split('').map(x => x+x).join('');
          const r = parseInt(hex.substring(0, 2), 16);
          const g = parseInt(hex.substring(2, 4), 16);
          const b = parseInt(hex.substring(4, 6), 16);
          const luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
          isLight = luma > 128;
        }

        return (
          <span 
            key={i} 
            style={{ 
              backgroundColor: part, 
              color: isLight ? '#000' : '#fff', 
              padding: '1px 4px', 
              borderRadius: '4px',
              border: '1px solid rgba(255,255,255,0.1)',
              fontWeight: 'bold'
            }}
          >
            {part}
          </span>
        );
      }
      return <span key={i}>{part}</span>;
    });
  };

  return (
    <div className="templates-page" style={{ padding: 24, display: 'flex', flexDirection: 'column', gap: 24, height: '100%', width: '100%' }}>
      {previewName && (
        <div className="modal-overlay" onClick={() => setPreviewName(null)}>
          <div className="modal-content" style={{ width: '80%', maxWidth: 800, maxHeight: '80vh', display: 'flex', flexDirection: 'column' }} onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <h3>Preview: {previewName}</h3>
              <X size={20} cursor="pointer" onClick={() => setPreviewName(null)} />
            </div>
            <div style={{ flex: 1, overflowY: 'auto', background: '#1e1e1e', padding: 16, borderRadius: 8, marginTop: 16 }}>
              {isPreviewLoading ? (
                <p style={{ color: '#888' }}>Rendering template with current colors...</p>
              ) : (
                <pre style={{ margin: 0, color: '#d4d4d4', fontFamily: 'monospace', fontSize: 13, whiteSpace: 'pre-wrap' }}>
                  {colorizeText(previewContent || '')}
                </pre>
              )}
            </div>
            <div className="modal-actions" style={{ marginTop: 16 }}>
              <button className="btn btn-secondary" onClick={() => setPreviewName(null)}>Close</button>
              <button className="btn btn-primary" disabled={isPreviewLoading}>Install Template</button>
            </div>
          </div>
        </div>
      )}

      {installTemplate && (
        <div className="modal-overlay" onClick={() => setInstallTemplate(null)}>
          <div className="modal-content" style={{ width: '90%', maxWidth: 500, display: 'flex', flexDirection: 'column' }} onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <h3>{t('templates.installTitle', { name: installTemplate.displayName })}</h3>
              <X size={20} cursor="pointer" onClick={() => setInstallTemplate(null)} />
            </div>
            <div style={{ marginTop: 16, display: 'flex', flexDirection: 'column', gap: 16 }}>
              <p style={{ color: 'var(--text-secondary)', fontSize: 14 }}>
                {t('templates.installDesc', { path: installTemplate.relativePath })}
              </p>
              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                <span className="template-badge">{installTemplate.category}</span>
                <span className="template-badge" style={{ color: automationColor(installTemplate.automationLevel), borderColor: automationColor(installTemplate.automationLevel) }}>
                  {automationLabel(installTemplate.automationLevel)}
                </span>
                {installTemplate.requiredCommands.map(cmd => (
                  <span key={cmd} className="template-badge">{t('templates.needs', { cmd })}</span>
                ))}
              </div>
              
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <label style={{ fontSize: 13, color: 'var(--text-secondary)' }}>{t('templates.outputPath')}</label>
                <input 
                  type="text" 
                  value={outputPath}
                  onChange={(e) => setOutputPath(e.target.value)}
                  placeholder="~/.config/alacritty/colors.toml"
                  style={{ padding: '10px 12px', borderRadius: 8, border: '1px solid var(--border)', background: 'var(--surface-hover)', color: 'var(--text-primary)' }}
                />
              </div>

              {installTemplate.relativePath.endsWith("midnight-discord.css") && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 12, marginTop: 8, padding: 12, background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 12 }}>
                  <label style={{ fontSize: 13, color: 'var(--text-secondary)' }}>{t('templates.selectDiscordClient')}</label>
                  <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                    {discordClients.map((client, idx) => (
                      <button
                        key={idx}
                        className="btn btn-compact"
                        style={{ 
                          flex: 1, 
                          background: outputPath.includes(client.path.replace('~/', '')) ? 'var(--accent)' : 'var(--surface-hover)',
                          color: outputPath.includes(client.path.replace('~/', '')) ? 'var(--on-accent)' : (client.exists ? '#4ade80' : '#f87171'),
                          border: `1px solid ${client.exists ? '#4ade80' : '#f87171'}`
                        }}
                        onClick={() => setOutputPath(`${client.path}/themes/matugen.css`)}
                      >
                        {client.name}
                      </button>
                    ))}
                  </div>
                  <span style={{ fontSize: 11, color: 'var(--text-secondary)', textAlign: 'center' }}>
                    {t('templates.discordHelp')}
                  </span>
                </div>
              )}

              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <label style={{ fontSize: 13, color: 'var(--text-secondary)' }}>{t('templates.postHook')}</label>
                <input 
                  type="text" 
                  value={postHook}
                  onChange={(e) => setPostHook(e.target.value)}
                  placeholder="e.g. killall -SIGUSR1 alacritty"
                  style={{ padding: '10px 12px', borderRadius: 8, border: '1px solid var(--border)', background: 'var(--surface-hover)', color: 'var(--text-primary)' }}
                />
              </div>

              {installTemplate.manualSteps.length > 0 && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 8, padding: 12, border: '1px solid var(--border)', borderRadius: 10, background: 'rgba(255,255,255,0.035)' }}>
                  <label style={{ fontSize: 13, color: 'var(--text-secondary)' }}>{t('templates.manualStepsNeeded')}</label>
                  {installTemplate.manualSteps.map((step, idx) => (
                    <span key={idx} style={{ fontSize: 12, color: 'var(--text-secondary)', lineHeight: 1.4 }}>{idx + 1}. {t(step, { defaultValue: step })}</span>
                  ))}
                </div>
              )}
            </div>
            <div className="modal-actions" style={{ marginTop: 24 }}>
              <button className="btn btn-secondary" onClick={() => setInstallTemplate(null)}>{t('templates.cancel')}</button>
              <button className="btn btn-primary" disabled={isInstalling} onClick={submitInstall}>
                {isInstalling ? t('templates.installing') : t('templates.install')}
              </button>
            </div>
          </div>
        </div>
      )}

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h2>{t('templates.title')}</h2>
          <p style={{ color: 'var(--text-secondary)' }}>{t('templates.subtitle')}</p>
        </div>
        
        <div style={{ position: 'relative' }}>
          <Search size={18} style={{ position: 'absolute', left: 12, top: 10, color: 'var(--text-secondary)' }} />
          <input 
            type="text" 
            placeholder={t('templates.searchPlaceholder')}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            style={{ 
              padding: '10px 10px 10px 40px', 
              borderRadius: 12, 
              border: '1px solid var(--border)', 
              background: 'var(--surface-hover)',
              color: 'var(--text-primary)',
              outline: 'none',
              width: 250
            }} 
          />
        </div>
      </div>

      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        {categories.map(category => (
          <button
            key={category}
            className={`btn btn-compact ${activeCategory === category ? 'btn-primary' : 'btn-secondary'}`}
            onClick={() => setActiveCategory(category)}
          >
            {category}
          </button>
        ))}
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: 16, overflowY: 'auto' }}>
        {isLoading ? (
          <p>{t('templates.loading')}</p>
        ) : filteredTemplates.length === 0 ? (
          <p>{t('templates.noTemplates')}</p>
        ) : (
          filteredTemplates.map((template, idx) => (
            <div key={idx} className="template-card" style={{
              background: 'var(--surface)',
              borderRadius: 16,
              padding: 20,
              border: '1px solid var(--border)',
              display: 'flex',
              flexDirection: 'column',
              gap: 16,
              cursor: 'pointer',
              transition: 'all 0.2s'
            }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                <div style={{ background: 'var(--accent-transparent)', padding: 12, borderRadius: 12, color: 'var(--accent)' }}>
                  <LayoutTemplate size={24} />
                </div>
                <div style={{ minWidth: 0 }}>
                  <h3 style={{ fontSize: 16, margin: 0 }}>{template.displayName}</h3>
                  <span style={{ fontSize: 12, color: 'var(--text-secondary)' }}>
                    {template.targetApp} • {(template.size / 1024).toFixed(1)} KB
                  </span>
                </div>
              </div>
              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                <span className="template-badge">{template.category}</span>
                <span className="template-badge" style={{ color: automationColor(template.automationLevel), borderColor: automationColor(template.automationLevel) }}>
                  {automationLabel(template.automationLevel)}
                </span>
                {!template.installable && <span className="template-badge">{t('templates.asset')}</span>}
              </div>
              <span style={{ fontSize: 12, color: 'var(--text-muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {template.relativePath}
              </span>
              <div style={{ display: 'flex', gap: 8, marginTop: 'auto' }}>
                <button className="btn btn-secondary btn-compact" style={{ flex: 1 }} onClick={() => handlePreview(template)}>{t('templates.preview')}</button>
                {installedTemplates.has(installedKey(template.name)) ? (
                  <button className="btn btn-danger btn-compact" style={{ flex: 1 }} onClick={() => handleUninstall(template)}>{t('templates.remove')}</button>
                ) : (
                  <button className="btn btn-primary btn-compact" style={{ flex: 1 }} disabled={!template.installable} onClick={() => handleInstallClick(template)}>
                    {template.installable ? t('templates.install') : t('templates.guide')}
                  </button>
                )}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
