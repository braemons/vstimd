import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The `/elements/` contract's bundle: one self-contained ES module, React and
// all -- no CDN, no external script tag, matching the same rule the console
// and the other two daemons follow. A second, separate config rather than a
// second entry in vite.config.ts because library mode and the main app's
// HTML-driven build target different things (one file vs. an app shell) and
// Vite does not mix the two in one `build` call.
//
// Run after the main `vite build`: that one empties `dist/` first, and this
// one only adds to it (`emptyOutDir: false`, and its own `dist/elements`
// subdirectory besides). See package.json's `build` script for the order.
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "dist/elements",
    emptyOutDir: false,
    lib: {
      entry: fileURLToPath(new URL("src/elements/vstimd_elements.tsx", import.meta.url)),
      formats: ["es"],
      fileName: () => "vstimd.js",
    },
  },
});
