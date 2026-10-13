/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        ink: "var(--bg)",
        elevated: "var(--bg-elevated)",
        panel: "var(--panel)",
        line: "var(--border)",
        accent: "var(--accent)",
        bright: "var(--accent-bright)",
        soft: "var(--accent-button)",
        inktext: "var(--accent-ink)",
        foreground: "var(--text)",
        muted: "var(--muted)",
        danger: "var(--danger)",
        ok: "var(--ok)",
        warn: "var(--warn)",
      },
      borderRadius: {
        card: "var(--radius)",
      },
      boxShadow: {
        lift: "var(--shadow)",
      },
      fontFamily: {
        sans: ['"Segoe UI Variable"', '"Segoe UI"', "system-ui", "sans-serif"],
      },
    },
  },
  plugins: [],
};
