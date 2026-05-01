import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getDefaultConfig } from "../utils/templateDefaults";
import { Search, LayoutTemplate, X } from "lucide-react";

interface TemplateInfo {
  name: String;
  path: String;
  size: number;
}

interface TemplatesProps {
  schemeData: any;
}

export default function Templates({ schemeData }: TemplatesProps) {
  const [templates, setTemplates] = useState<TemplateInfo[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [previewContent, setPreviewContent] = useState<string | null>(null);
  const [previewName, setPreviewName] = useState<string | null>(null);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);
  const [installTemplate, setInstallTemplate] = useState<TemplateInfo | null>(null);
  const [outputPath, setOutputPath] = useState("");
  const [postHook, setPostHook] = useState("");
  const [isInstalling, setIsInstalling] = useState(false);
  const [discordClients, setDiscordClients] = useState<any[]>([]);

  // We add an effect to check Discord clients when the modal opens for midnight-discord.css
  useEffect(() => {
    if (installTemplate && installTemplate.name === "midnight-discord.css") {
      checkDiscordClients();
    }
  }, [installTemplate]);

  const checkDiscordClients = async () => {
    const clients = [
      { name: "Vesktop", path: "~/.config/vesktop" },
      { name: "Equibop", path: "~/.config/equibop" },
      { name: "Vencord", path: "~/.config/Vencord" }
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
        const data: TemplateInfo[] = await invoke("list_available_templates", {
          themesDir: "/home/raell/Projetos/matugen-themes"
        });
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

  const filteredTemplates = templates.filter(t => 
    t.name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const formatTemplateName = (filename: String) => {
    // Convert to string and remove extension
    const nameWithoutExt = filename.toString().replace(/\.[^/.]+$/, "");
    // Replace hyphens/underscores with spaces
    const spacedName = nameWithoutExt.replace(/[-_]/g, " ");
    // Capitalize each word
    return spacedName.replace(/\b\w/g, char => char.toUpperCase());
  };

  const handlePreview = async (template: TemplateInfo) => {
    if (!schemeData) {
      alert("Please extract colors from an image first in the Colors tab!");
      return;
    }
    
    setIsPreviewLoading(true);
    setPreviewName(formatTemplateName(template.name));
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
    const tName = template.name.toString();
    const config = getDefaultConfig(tName);
    
    setInstallTemplate(template);
    setOutputPath(config.outputPath);
    setPostHook(config.postHook || "");
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
      alert(`Template ${installTemplate.name} installed successfully!`);
      setInstallTemplate(null);
      fetchInstalled();
    } catch (err: any) {
      alert(`Error installing template:\n${err}`);
    } finally {
      setIsInstalling(false);
    }
  };

  const handleUninstall = async (template: TemplateInfo) => {
    if (confirm(`Are you sure you want to remove ${template.name} from your active templates?`)) {
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
              <h3>Install {formatTemplateName(installTemplate.name)}</h3>
              <X size={20} cursor="pointer" onClick={() => setInstallTemplate(null)} />
            </div>
            <div style={{ marginTop: 16, display: 'flex', flexDirection: 'column', gap: 16 }}>
              <p style={{ color: 'var(--text-secondary)', fontSize: 14 }}>
                This will copy the template to your matugen templates directory and add it to <code>config.toml</code>.
              </p>
              
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <label style={{ fontSize: 13, color: 'var(--text-secondary)' }}>Output Path</label>
                <input 
                  type="text" 
                  value={outputPath}
                  onChange={(e) => setOutputPath(e.target.value)}
                  placeholder="~/.config/alacritty/colors.toml"
                  style={{ padding: '10px 12px', borderRadius: 8, border: '1px solid var(--border)', background: 'var(--surface-hover)', color: 'var(--text-primary)' }}
                />
              </div>

              {installTemplate.name === "midnight-discord.css" && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 12, marginTop: 8, padding: 12, background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 12 }}>
                  <label style={{ fontSize: 13, color: 'var(--text-secondary)' }}>Select Discord Client</label>
                  <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                    {discordClients.map((client, idx) => (
                      <button 
                        key={idx}
                        className="btn"
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
                    Green = Detected. Red = Not found. You can install even if not found.
                  </span>
                </div>
              )}

              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <label style={{ fontSize: 13, color: 'var(--text-secondary)' }}>Post Hook (Optional)</label>
                <input 
                  type="text" 
                  value={postHook}
                  onChange={(e) => setPostHook(e.target.value)}
                  placeholder="e.g. killall -SIGUSR1 alacritty"
                  style={{ padding: '10px 12px', borderRadius: 8, border: '1px solid var(--border)', background: 'var(--surface-hover)', color: 'var(--text-primary)' }}
                />
              </div>
            </div>
            <div className="modal-actions" style={{ marginTop: 24 }}>
              <button className="btn btn-secondary" onClick={() => setInstallTemplate(null)}>Cancel</button>
              <button className="btn btn-primary" disabled={isInstalling} onClick={submitInstall}>
                {isInstalling ? "Installing..." : "Install"}
              </button>
            </div>
          </div>
        </div>
      )}

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h2>Template Gallery</h2>
          <p style={{ color: 'var(--text-secondary)' }}>Manage your application templates</p>
        </div>
        
        <div style={{ position: 'relative' }}>
          <Search size={18} style={{ position: 'absolute', left: 12, top: 10, color: 'var(--text-secondary)' }} />
          <input 
            type="text" 
            placeholder="Search templates..." 
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

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: 16, overflowY: 'auto' }}>
        {isLoading ? (
          <p>Loading templates...</p>
        ) : filteredTemplates.length === 0 ? (
          <p>No templates found.</p>
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
                <div>
                  <h3 style={{ fontSize: 16, margin: 0 }}>{formatTemplateName(template.name)}</h3>
                  <span style={{ fontSize: 12, color: 'var(--text-secondary)' }}>
                    {(template.size / 1024).toFixed(1)} KB • {template.name.toString().split('.').pop()?.toUpperCase()}
                  </span>
                </div>
              </div>
              <div style={{ display: 'flex', gap: 8, marginTop: 'auto' }}>
                <button className="btn btn-secondary" style={{ flex: 1, padding: 8, fontSize: 13 }} onClick={() => handlePreview(template)}>Preview</button>
                {installedTemplates.has(template.name.toString().replace(/\./g, "_").replace(/-/g, "_")) ? (
                  <button className="btn btn-secondary" style={{ flex: 1, padding: 8, fontSize: 13, background: 'rgba(255,100,100,0.1)', color: '#ff6b6b' }} onClick={() => handleUninstall(template)}>Remove</button>
                ) : (
                  <button className="btn btn-primary" style={{ flex: 1, padding: 8, fontSize: 13 }} onClick={() => handleInstallClick(template)}>Install</button>
                )}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
