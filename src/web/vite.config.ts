import tailwindcss from '@tailwindcss/vite'
import path from "path"
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    // Allow Tailscale HTTPS / MagicDNS hostnames like `device.tailxxxx.ts.net`
    // Vite treats entries starting with "." as "this domain + all subdomains".
    allowedHosts: ['.ts.net'],
  },
})
