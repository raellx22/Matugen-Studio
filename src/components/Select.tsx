import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown } from "lucide-react";

export interface SelectOption { value: string; label: string; group?: string; disabled?: boolean }
interface Props {
  value: string;
  options: SelectOption[];
  onValueChange: (value: string) => void;
  disabled?: boolean;
  label?: string;
  className?: string;
}

export default function Select({ value, options, onValueChange, disabled, label, className = "" }: Props) {
  const id = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [position, setPosition] = useState({ top: 0, left: 0, width: 0, maxHeight: 240 });
  const available = options.map((option, index) => option.disabled ? -1 : index).filter(index => index >= 0);
  const selected = options.find(option => option.value === value);

  const place = () => {
    const rect = trigger.current?.getBoundingClientRect();
    if (!rect) return;
    const gap = 6;
    const below = window.innerHeight - rect.bottom - gap - 10;
    const above = rect.top - gap - 10;
    const desired = Math.min(320, options.length * 42 + 20);
    const up = below < Math.min(desired, 170) && above > below;
    const maxHeight = Math.max(72, Math.min(320, up ? above : below));
    setPosition({
      top: up ? rect.top - gap - Math.min(desired, maxHeight) : rect.bottom + gap,
      left: Math.max(8, Math.min(rect.left, window.innerWidth - Math.max(rect.width, 160) - 8)),
      width: Math.min(Math.max(rect.width, 160), window.innerWidth - 16),
      maxHeight,
    });
  };

  useLayoutEffect(() => { if (open) place(); }, [open, options.length]);
  useEffect(() => {
    if (!open) return;
    const onPointer = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!trigger.current?.contains(target) && !menu.current?.contains(target)) setOpen(false);
    };
    const onScroll = (event: Event) => { if (!menu.current?.contains(event.target as Node)) place(); };
    const onResize = () => place();
    document.addEventListener("pointerdown", onPointer);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onResize);
    return () => {
      document.removeEventListener("pointerdown", onPointer);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onResize);
    };
  }, [open, options.length]);
  useEffect(() => { if (open) menu.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" }); }, [active, open]);

  const select = (index: number) => {
    const option = options[index];
    if (!option || option.disabled) return;
    onValueChange(option.value);
    setOpen(false);
    trigger.current?.focus();
  };
  const onKeyDown = (event: React.KeyboardEvent) => {
    if (disabled || available.length === 0) return;
    if (event.key === "Escape") { if (open) { event.preventDefault(); setOpen(false); trigger.current?.focus(); } return; }
    if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      if (!open) { setActive(Math.max(0, options.findIndex(option => option.value === value))); setOpen(true); return; }
      const cursor = available.indexOf(active);
      const next = event.key === "Home" ? 0 : event.key === "End" ? available.length - 1 :
        event.key === "ArrowDown" ? (cursor + 1) % available.length : (cursor - 1 + available.length) % available.length;
      setActive(available[next]);
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (open) select(active);
      else { setActive(Math.max(0, options.findIndex(option => option.value === value))); setOpen(true); }
    } else if (event.key === "Tab") setOpen(false);
  };

  return <>
    <button ref={trigger} type="button" role="combobox" aria-label={label} aria-expanded={open}
      aria-controls={open ? id : undefined} aria-haspopup="listbox" aria-activedescendant={open ? `${id}-${active}` : undefined}
      className={`studio-select ${className}`} disabled={disabled} onKeyDown={onKeyDown}
      onClick={() => { setActive(Math.max(0, options.findIndex(option => option.value === value))); setOpen(current => !current); }}>
      <span>{selected?.label ?? value}</span><ChevronDown size={16} aria-hidden="true" />
    </button>
    {open && createPortal(<div ref={menu} id={id} role="listbox" aria-label={label}
      className="studio-select-menu" onKeyDown={onKeyDown} tabIndex={-1}
      style={{ top: position.top, left: position.left, width: position.width, maxHeight: position.maxHeight }}>
      {options.map((option, index) => <div key={`${option.value}-${index}`}>
        {option.group && (index === 0 || options[index - 1]?.group !== option.group) && <div className="studio-select-group">{option.group}</div>}
        <div id={`${id}-${index}`} data-index={index} role="option" aria-label={option.group ? `${option.group}: ${option.label}` : option.label} aria-selected={option.value === value}
          aria-disabled={option.disabled || undefined} className={`studio-select-option ${active === index ? "is-active" : ""}`}
          onMouseEnter={() => { if (!option.disabled) setActive(index); }}
          onClick={() => select(index)}>
          <span>{option.label}</span>{option.value === value && <Check size={16} aria-hidden="true" />}
        </div>
      </div>)}
    </div>, document.body)}
  </>;
}
