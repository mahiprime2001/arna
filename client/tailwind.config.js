/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        canvas: "hsl(var(--canvas))",
        surface: "hsl(var(--surface))",
        elevated: "hsl(var(--elevated))",
        line: "hsl(var(--line))",
        ink: "hsl(var(--ink))",
        muted: "hsl(var(--muted))",
        brand: {
          DEFAULT: "hsl(var(--brand))",
          fg: "hsl(var(--brand-fg))",
          soft: "hsl(var(--brand) / 0.12)",
        },
        good: "hsl(var(--good))",
        danger: "hsl(var(--danger))",
      },
      borderColor: { DEFAULT: "hsl(var(--line))" },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 4px)",
        sm: "calc(var(--radius) - 8px)",
      },
      fontFamily: {
        sans: ["'Geist Variable'", "system-ui", "sans-serif"],
        mono: ["'Geist Mono Variable'", "ui-monospace", "monospace"],
      },
      boxShadow: {
        card: "0 1px 2px hsl(240 30% 4% / 0.04), 0 8px 24px -12px hsl(240 30% 4% / 0.12)",
        pop: "0 8px 30px -8px hsl(240 30% 4% / 0.28)",
      },
      keyframes: {
        "fade-up": {
          from: { opacity: "0", transform: "translateY(6px)" },
          to: { opacity: "1", transform: "translateY(0)" },
        },
      },
      animation: { "fade-up": "fade-up 0.35s cubic-bezier(0.16,1,0.3,1) both" },
    },
  },
  plugins: [],
};
