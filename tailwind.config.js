/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: ["Inter", "ui-sans-serif", "system-ui", "sans-serif"],
        mono: ["JetBrains Mono", "ui-monospace", "SFMono-Regular", "monospace"],
      },
      colors: {
        accent: {
          50: "#eef8f4",
          100: "#d8eee6",
          600: "#2f7d6d",
          700: "#28685c",
        },
      },
    },
  },
  plugins: [require("@tailwindcss/typography")],
};
