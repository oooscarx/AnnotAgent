/** AnnotAgent Tailwind preset. Import it from your existing Tailwind config; do not replace the whole config. */
module.exports = {
  theme: {
    extend: {
      colors: {
        aa: {
          ink: '#0D1117', navy: '#07111F', primary: '#2563EB', teal: '#00B3A4', violet: '#7C3AED',
          bg: 'var(--aa-bg)', surface: 'var(--aa-surface)', muted: 'var(--aa-surface-muted)', text: 'var(--aa-text)', border: 'var(--aa-border)',
          success: '#16A34A', warning: '#D97706', danger: '#DC2626', info: '#0284C7',
        },
      },
      borderRadius: { 'aa-sm': 'var(--aa-radius-sm)', 'aa': 'var(--aa-radius-md)', 'aa-lg': 'var(--aa-radius-lg)' },
      boxShadow: { 'aa-sm': 'var(--aa-shadow-sm)', 'aa': 'var(--aa-shadow-md)', 'aa-focus': 'var(--aa-focus-ring)' },
      fontFamily: { sans: ['Inter', 'ui-sans-serif', 'system-ui', 'sans-serif'], mono: ['JetBrains Mono', 'ui-monospace', 'monospace'] },
    },
  },
};
