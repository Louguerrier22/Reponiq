export type RegistryTheme = {
  name: string;
  light: Record<string, string>;
  dark: Record<string, string>;
};

export type Theme = {
  preset: 'reponiq' | 'nova' | 'lyra';
  mode: 'light' | 'dark' | 'system';
  palette: 'graphite' | 'neutral' | 'slate' | null;
  accent: 'lavender' | 'indigo' | 'teal' | 'rose' | null;
  radius: 'square' | 'soft' | 'rounded' | null;
  density: 'compact' | 'comfortable' | null;
  text_size: 'standard' | 'large' | null;
  imported?: RegistryTheme;
};

export type SetupStep = 'appearance' | 'workspace' | 'accounts' | 'startup' | 'review' | 'complete';
export type AppearanceSettings = { theme: Theme; setup_step: SetupStep };

export const presets = {
  reponiq: { label: 'Reponiq', palette: 'graphite', accent: 'lavender', radius: 'rounded', density: 'comfortable' },
  nova: { label: 'Nova', palette: 'neutral', accent: 'indigo', radius: 'soft', density: 'comfortable' },
  lyra: { label: 'Lyra', palette: 'slate', accent: 'lavender', radius: 'square', density: 'compact' },
} as const;

export const defaultTheme: Theme = {
  preset: 'reponiq', mode: 'dark', palette: null, accent: null, radius: null, density: null, text_size: null,
};

export function themeAttributes(theme: Theme, systemDark: boolean) {
  const preset = presets[theme.preset];
  const mode = theme.mode === 'system' ? (systemDark ? 'dark' : 'light') : theme.mode;
  return {
    'data-theme-mode': mode,
    'data-theme-palette': theme.palette ?? preset.palette,
    'data-theme-accent': theme.accent ?? preset.accent,
    'data-theme-radius': theme.radius ?? preset.radius,
    'data-theme-density': theme.density ?? preset.density,
    'data-theme-text': theme.text_size ?? 'standard',
    'data-theme-custom': mode === 'light' || theme.preset !== 'reponiq' || theme.palette !== null || theme.accent !== null || theme.imported ? '' : undefined,
  };
}

const sharedColors = ['background','foreground','card','popover','primary','primary-foreground','muted','muted-foreground','border','ring'] as const;
const registryColors = ['card-foreground','popover-foreground','secondary','secondary-foreground','accent','accent-foreground','input','destructive','sidebar','sidebar-foreground','sidebar-primary','sidebar-primary-foreground','sidebar-accent','sidebar-accent-foreground','sidebar-border','sidebar-ring'] as const;
export const registryVariables = [...sharedColors.map(key => '--'+key), ...registryColors.map(key => '--registry-'+key), '--radius'];

export function themeVariables(theme: Theme, systemDark: boolean): Record<string, string> {
  if (!theme.imported) return {};
  const mode = themeAttributes(theme,systemDark)['data-theme-mode'];
  const colors = theme.imported[mode];
  const entries = sharedColors.filter(key=>colors[key]).map(key=>['--'+key,colors[key]]);
  entries.push(...registryColors.filter(key=>colors[key]).map(key=>['--registry-'+key,colors[key]]));
  if (theme.radius===null && colors.radius) entries.push(['--radius',colors.radius]);
  return Object.fromEntries(entries);
}
