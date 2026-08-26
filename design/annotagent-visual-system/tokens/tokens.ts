export const annotAgentTokens = {
  "$schema": "./tokens.schema.json",
  "name": "AnnotAgent Visual System",
  "version": "1.0.0",
  "color": {
    "brand": {
      "ink": "#0D1117",
      "navy950": "#07111F",
      "navy900": "#0B172A",
      "primary": "#2563EB",
      "primaryHover": "#1D4ED8",
      "teal": "#00B3A4",
      "tealText": "#0F766E",
      "violet": "#7C3AED"
    },
    "light": {
      "background": "#F8FAFC",
      "surface": "#FFFFFF",
      "surfaceMuted": "#F1F5F9",
      "text": "#0D1117",
      "textMuted": "#64748B",
      "border": "#E2E8F0"
    },
    "dark": {
      "background": "#07111F",
      "surface": "#0B172A",
      "surfaceMuted": "#12243A",
      "text": "#F8FAFC",
      "textMuted": "#94A3B8",
      "border": "#263A52"
    },
    "semantic": {
      "success": "#16A34A",
      "warning": "#D97706",
      "danger": "#DC2626",
      "info": "#0284C7"
    },
    "annotation": {
      "slot1": "#2563EB",
      "slot2": "#00A896",
      "slot3": "#7C3AED",
      "slot4": "#F59E0B",
      "slot5": "#E11D48",
      "slot6": "#16A34A",
      "slot7": "#0EA5E9",
      "slot8": "#F97316"
    }
  },
  "typography": {
    "sans": "Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
    "mono": "'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
    "weight": {
      "regular": 400,
      "medium": 500,
      "semibold": 600,
      "bold": 700
    },
    "size": {
      "xs": "0.75rem",
      "sm": "0.875rem",
      "md": "1rem",
      "lg": "1.25rem",
      "xl": "1.5rem",
      "2xl": "2rem",
      "display": "3rem"
    },
    "lineHeight": {
      "tight": 1.2,
      "normal": 1.5,
      "relaxed": 1.65
    }
  },
  "spacing": {
    "0": "0",
    "1": "0.25rem",
    "2": "0.5rem",
    "3": "0.75rem",
    "4": "1rem",
    "6": "1.5rem",
    "8": "2rem",
    "12": "3rem",
    "16": "4rem"
  },
  "radius": {
    "sm": "0.375rem",
    "md": "0.625rem",
    "lg": "0.875rem",
    "xl": "1.125rem",
    "pill": "999px"
  },
  "shadow": {
    "sm": "0 1px 2px rgba(13,17,23,.06)",
    "md": "0 8px 24px rgba(13,17,23,.10)",
    "focus": "0 0 0 3px rgba(37,99,235,.25)"
  },
  "motion": {
    "fast": "120ms",
    "normal": "180ms",
    "slow": "240ms",
    "easing": "cubic-bezier(.2,.8,.2,1)"
  },
  "layout": {
    "sidebarWidth": "15rem",
    "inspectorWidth": "21rem",
    "contentMax": "96rem"
  }
} as const;

export type AnnotAgentTokens = typeof annotAgentTokens;
export type AnnotationColorSlot = keyof typeof annotAgentTokens.color.annotation;
