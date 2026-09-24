export interface GenerationSettings {
  seed_index: number;
  contrast: number;
  chroma: number;
  tone: number;
  material_spec: "2021" | "2025";
}
export interface KdeIntegrationSettings {
  klassy: { enabled: boolean; outline_sync: boolean; titlebar_opacity: number | null };
  rounded_corners: { enabled: boolean; outline_sync: boolean; outline_source: "automatic" | "primary" | "outline" | "outline_variant" };
}
export interface StudioSettings {
  generation: GenerationSettings;
  integrations: KdeIntegrationSettings;
  mode: "dark" | "light" | "auto";
  scheme_type: string;
  kde_overrides: Record<string, string>;
  gtk_enabled: boolean | null;
}
export const defaultGeneration: GenerationSettings = { seed_index: 0, contrast: 0, chroma: 1, tone: 1, material_spec: "2025" };
export const defaultIntegrations: KdeIntegrationSettings = {
  klassy: { enabled: false, outline_sync: false, titlebar_opacity: null },
  rounded_corners: { enabled: false, outline_sync: false, outline_source: "automatic" },
};
export const defaultStudioSettings: StudioSettings = { generation: defaultGeneration, integrations: defaultIntegrations, mode: "auto", scheme_type: "Tinted Smart", kde_overrides: {}, gtk_enabled: null };
