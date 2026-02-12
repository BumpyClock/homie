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
      // Deduplicate React — pnpm hoists a different version to the workspace
      // root which causes "Invalid hook call" errors in Radix UI and other
      // libraries that resolve React from the root node_modules.
      "react": path.resolve(__dirname, "node_modules/react"),
      "react-dom": path.resolve(__dirname, "node_modules/react-dom"),
    },
    // Ensure Vite deduplicates React across all dependency trees
    dedupe: ["react", "react-dom"],
  },
  server: {
    // Allow Tailscale HTTPS / MagicDNS hostnames like `device.tailxxxx.ts.net`
    // Vite treats entries starting with "." as "this domain + all subdomains".
    allowedHosts: ['.ts.net'],
  },
})
