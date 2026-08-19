/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      // Every color resolves through a CSS custom property so the three theme
      // states (system / explicit light / explicit dark) share one definition.
      colors: {
        ground: "var(--ground)",
        surface: { DEFAULT: "var(--surface)", 2: "var(--surface-2)", 3: "var(--surface-3)" },
        ink: { DEFAULT: "var(--ink)", 2: "var(--ink-2)", 3: "var(--ink-3)" },
        line: { DEFAULT: "var(--line)", 2: "var(--line-2)" },
        accent: { DEFAULT: "var(--accent)", soft: "var(--accent-soft)", ink: "var(--accent-ink)" },
        alert: { DEFAULT: "var(--alert)", soft: "var(--alert-soft)" },
        rest: { DEFAULT: "var(--rest)", soft: "var(--rest-soft)", ink: "var(--rest-ink)" },
        warn: { DEFAULT: "var(--warn)", soft: "var(--warn-soft)" },
      },
      boxShadow: { pop: "var(--shadow)" },
      fontFamily: {
        sans: ["-apple-system", "BlinkMacSystemFont", "system-ui", "sans-serif"],
        mono: ["ui-monospace", "SFMono-Regular", "Menlo", "monospace"],
      },
    },
  },
  plugins: [],
};
