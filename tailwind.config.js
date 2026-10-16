/** @type {import('tailwindcss').Config} */
export default {
  darkMode: 'class',
  content: [
    "./index.html",
    "./src/**/*.{vue,js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      fontFamily: {
        sans: ['Inter', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'Roboto', 'sans-serif'],
        mono: ['"JetBrains Mono"', '"Fira Code"', 'Consolas', 'monospace'],
      },
      colors: {
        brand: {
          50: '#eefcf5',
          100: '#d7f7e6',
          200: '#b2edd0',
          300: '#7dddb3',
          400: '#43c591',
          500: '#10b981', // 翡翠青
          600: '#059669',
          700: '#047857',
          800: '#065f46',
          900: '#064e3b',
          accent: '#06b6d4',
        },
        surface: {
          base: '#0c0e14',     // 主底色
          sidebar: '#11141d',  // 侧边栏
          card: '#161a26',     // 卡片底色
          elevated: '#1d2333', // 浮动卡片/悬浮
          border: '#262d40',   // 边框色
          highlight: '#2f3850',// 选中/高亮
          subtle: '#8892b0',   // 次要文本
        }
      },
      boxShadow: {
        'glow-emerald': '0 0 25px -5px rgba(16, 185, 129, 0.35)',
        'glow-cyan': '0 0 25px -5px rgba(6, 182, 212, 0.35)',
        'glow-card': '0 10px 30px -10px rgba(0, 0, 0, 0.5), 0 0 1px 1px rgba(255, 255, 255, 0.05)',
        'glass': '0 8px 32px 0 rgba(0, 0, 0, 0.37)',
      }
    },
  },
  plugins: [],
}
