import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { RotateCcw } from "lucide-react";
import { defaultGeneration, type GenerationSettings } from "../utils/studioSettings";
import Select from "../components/Select";

interface Props {
  settings: GenerationSettings;
  candidates: { index: number; hex: string }[];
  selectedIndex: number;
  disabled: boolean;
  onChange: (settings: GenerationSettings) => Promise<void>;
}
export default function AdvancedGeneration({ settings, candidates, selectedIndex, disabled, onChange }: Props) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(settings);
  useEffect(() => setDraft(settings), [settings]);
  const change = (key: "contrast" | "chroma" | "tone", value: number) => setDraft(current => ({ ...current, [key]: value }));
  const dirty = JSON.stringify(draft) !== JSON.stringify(settings);
  return <div className="advanced-generation">
    {candidates.length > 0 && <div className="seed-candidates" role="group" aria-label={t("advanced.seedPicker")}>
      {candidates.map(candidate => <button type="button" key={candidate.index} className={`seed-candidate ${selectedIndex === candidate.index ? "selected" : ""}`}
        disabled={disabled} aria-pressed={selectedIndex === candidate.index}
        title={t("advanced.seedTooltip", { index: candidate.index, hex: candidate.hex })}
        onClick={() => void onChange({ ...settings, seed_index: candidate.index })}>
        <span className="seed-dot" style={{ backgroundColor: candidate.hex }} />
        <span>{candidate.index} · {candidate.hex.toUpperCase()}</span>
      </button>)}
    </div>}
    <details className="advanced-panel">
      <summary>{t("advanced.title")}</summary>
      <fieldset disabled={disabled}>
        <div className="advanced-control"><span>{t("advanced.spec")}</span>
          <Select label={t("advanced.spec")} value={draft.material_spec} onValueChange={value => setDraft({ ...draft, material_spec: value as GenerationSettings["material_spec"] })}
            options={[{ value: "2025", label: "2025" }, { value: "2021", label: "2021" }]} />
        </div>
        {([
          ["contrast", -1, 1, 0.05], ["chroma", 0, 10, 0.1], ["tone", 0, 1.5, 0.05],
        ] as const).map(([key, min, max, step]) => <label key={key} className="advanced-control" title={t(`advanced.${key}Help`)}>
          <span>{t(`advanced.${key}`)} <output>{draft[key].toFixed(2)}</output></span>
          <input type="range" min={min} max={max} step={step} value={draft[key]} onChange={event => change(key, event.target.valueAsNumber)} />
        </label>)}
        <p className="desktop-card-copy">{t("advanced.toneNote")}</p>
        <div className="advanced-actions">
          <button type="button" className="btn btn-primary btn-compact" disabled={!dirty} onClick={() => void onChange(draft)}>{t("advanced.apply")}</button>
          <button type="button" className="btn btn-secondary btn-compact" onClick={() => { setDraft(defaultGeneration); void onChange(defaultGeneration); }}><RotateCcw size={14} />{t("advanced.reset")}</button>
        </div>
      </fieldset>
    </details>
  </div>;
}
