export type ThemeCatalogItem = {
  name: string;
  source: "shadcn/ui" | "tweakcn";
  url: string;
  swatches: Record<"light" | "dark", [string, string, string]>;
};

// Samples are display metadata. The full palette is fetched only on request.
export const themeCatalog: ThemeCatalogItem[] = [
  {
    name: "Zinc",
    source: "shadcn/ui",
    url: "https://ui.shadcn.com/r/themes/zinc.json",
    swatches: {
      light: ["hsl(0 0% 100%)", "hsl(0 0% 100%)", "hsl(240 5.9% 10%)"],
      dark: ["hsl(240 10% 3.9%)", "hsl(240 10% 3.9%)", "hsl(0 0% 98%)"],
    },
  },
  {
    name: "Ocean Breeze",
    source: "tweakcn",
    url: "https://tweakcn.com/r/themes/ocean-breeze.json",
    swatches: {
      light: [
        "oklch(0.9751 0.0127 244.2507)",
        "oklch(1.0000 0 0)",
        "oklch(0.7227 0.1920 149.5793)",
      ],
      dark: [
        "oklch(0.2077 0.0398 265.7549)",
        "oklch(0.2795 0.0368 260.0310)",
        "oklch(0.7729 0.1535 163.2231)",
      ],
    },
  },
  {
    name: "Violet Bloom",
    source: "tweakcn",
    url: "https://tweakcn.com/r/themes/violet-bloom.json",
    swatches: {
      light: [
        "oklch(0.9940 0 0)",
        "oklch(0.9940 0 0)",
        "oklch(0.5393 0.2713 286.7462)",
      ],
      dark: [
        "oklch(0.2223 0.0060 271.1393)",
        "oklch(0.2568 0.0076 274.6528)",
        "oklch(0.6132 0.2294 291.7437)",
      ],
    },
  },
  {
    name: "Slate",
    source: "shadcn/ui",
    url: "https://ui.shadcn.com/r/themes/slate.json",
    swatches: {
      light: ["hsl(0 0% 100%)", "hsl(0 0% 100%)", "hsl(222.2 47.4% 11.2%)"],
      dark: ["hsl(222.2 84% 4.9%)", "hsl(222.2 84% 4.9%)", "hsl(210 40% 98%)"],
    },
  },
  {
    name: "Nature",
    source: "tweakcn",
    url: "https://tweakcn.com/r/themes/nature.json",
    swatches: {
      light: [
        "oklch(0.9711 0.0074 80.7211)",
        "oklch(0.9711 0.0074 80.7211)",
        "oklch(0.5234 0.1347 144.1672)",
      ],
      dark: [
        "oklch(0.2683 0.0279 150.7681)",
        "oklch(0.3327 0.0271 146.9867)",
        "oklch(0.6731 0.1624 144.2083)",
      ],
    },
  },
  {
    name: "Caffeine",
    source: "tweakcn",
    url: "https://tweakcn.com/r/themes/caffeine.json",
    swatches: {
      light: [
        "oklch(0.9821 0 0)",
        "oklch(0.9911 0 0)",
        "oklch(0.4341 0.0392 41.9938)",
      ],
      dark: [
        "oklch(0.1776 0 0)",
        "oklch(0.2134 0 0)",
        "oklch(0.9247 0.0524 66.1732)",
      ],
    },
  },
  {
    name: "Stone",
    source: "shadcn/ui",
    url: "https://ui.shadcn.com/r/themes/stone.json",
    swatches: {
      light: ["hsl(0 0% 100%)", "hsl(0 0% 100%)", "hsl(24 9.8% 10%)"],
      dark: ["hsl(20 14.3% 4.1%)", "hsl(20 14.3% 4.1%)", "hsl(60 9.1% 97.8%)"],
    },
  },
  {
    name: "Pastel Dreams",
    source: "tweakcn",
    url: "https://tweakcn.com/r/themes/pastel-dreams.json",
    swatches: {
      light: [
        "oklch(0.9689 0.0090 314.7819)",
        "oklch(1.0000 0 0)",
        "oklch(0.7090 0.1592 293.5412)",
      ],
      dark: [
        "oklch(0.2161 0.0061 56.0434)",
        "oklch(0.2805 0.0309 307.2326)",
        "oklch(0.7874 0.1179 295.7538)",
      ],
    },
  },
  {
    name: "Midnight Bloom",
    source: "tweakcn",
    url: "https://tweakcn.com/r/themes/midnight-bloom.json",
    swatches: {
      light: [
        "oklch(0.9821 0 0)",
        "oklch(1.0000 0 0)",
        "oklch(0.5676 0.2021 283.0838)",
      ],
      dark: [
        "oklch(0.2303 0.0125 264.2926)",
        "oklch(0.3210 0.0078 223.6661)",
        "oklch(0.5676 0.2021 283.0838)",
      ],
    },
  },
  {
    name: "Neutral",
    source: "shadcn/ui",
    url: "https://ui.shadcn.com/r/themes/neutral.json",
    swatches: {
      light: ["hsl(0 0% 100%)", "hsl(0 0% 100%)", "hsl(0 0% 9%)"],
      dark: ["hsl(0 0% 3.9%)", "hsl(0 0% 3.9%)", "hsl(0 0% 98%)"],
    },
  },
  {
    name: "Gray",
    source: "shadcn/ui",
    url: "https://ui.shadcn.com/r/themes/gray.json",
    swatches: {
      light: ["hsl(0 0% 100%)", "hsl(0 0% 100%)", "hsl(220.9 39.3% 11%)"],
      dark: ["hsl(224 71.4% 4.1%)", "hsl(224 71.4% 4.1%)", "hsl(210 20% 98%)"],
    },
  },
  {
    name: "Modern Minimal",
    source: "tweakcn",
    url: "https://tweakcn.com/r/themes/modern-minimal.json",
    swatches: {
      light: [
        "oklch(1.0000 0 0)",
        "oklch(1.0000 0 0)",
        "oklch(0.6231 0.1880 259.8145)",
      ],
      dark: [
        "oklch(0.2046 0 0)",
        "oklch(0.2686 0 0)",
        "oklch(0.6231 0.1880 259.8145)",
      ],
    },
  },
  {
    name: "Claude",
    source: "tweakcn",
    url: "https://tweakcn.com/r/themes/claude.json",
    swatches: {
      light: [
        "oklch(0.9818 0.0054 95.0986)",
        "oklch(0.9818 0.0054 95.0986)",
        "oklch(0.6171 0.1375 39.0427)",
      ],
      dark: [
        "oklch(0.2679 0.0036 106.6427)",
        "oklch(0.2679 0.0036 106.6427)",
        "oklch(0.6724 0.1308 38.7559)",
      ],
    },
  },
  {
    name: "Neo Brutalism",
    source: "tweakcn",
    url: "https://tweakcn.com/r/themes/neo-brutalism.json",
    swatches: {
      light: [
        "oklch(1.0000 0 0)",
        "oklch(1.0000 0 0)",
        "oklch(0.6489 0.2370 26.9728)",
      ],
      dark: [
        "oklch(0 0 0)",
        "oklch(0.3211 0 0)",
        "oklch(0.7044 0.1872 23.1858)",
      ],
    },
  },
];
