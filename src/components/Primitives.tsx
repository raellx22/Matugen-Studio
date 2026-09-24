import type { ReactNode } from "react";

export function PageHeader({ title, description, actions }: { title: string; description?: string; actions?: ReactNode }) {
  return <header className="page-header"><div><h2>{title}</h2>{description && <p>{description}</p>}</div>{actions && <div className="page-actions">{actions}</div>}</header>;
}

export function Section({ title, description, actions, children, className = "" }: { title: string; description?: string; actions?: ReactNode; children: ReactNode; className?: string }) {
  return <section className={`studio-section ${className}`}><header className="studio-section-header"><div><h3>{title}</h3>{description && <p>{description}</p>}</div>{actions}</header><div className="studio-section-body">{children}</div></section>;
}

export function SettingRow({ title, description, children }: { title: string; description?: ReactNode; children: ReactNode }) {
  return <div className="setting-row"><div className="setting-row-copy"><h4>{title}</h4>{description && <p>{description}</p>}</div><div className="setting-row-control">{children}</div></div>;
}

export function Switch({ checked, onChange, disabled, label }: { checked: boolean; onChange: (checked: boolean) => void; disabled?: boolean; label: string }) {
  return <button type="button" role="switch" aria-checked={checked} aria-label={label} disabled={disabled}
    className={`studio-switch ${checked ? "is-on" : ""}`} onClick={() => onChange(!checked)}><span /></button>;
}
