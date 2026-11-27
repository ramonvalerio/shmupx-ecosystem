import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { execSync } from "node:child_process";
import type { Plugin } from "vite";

/**
 * Versão da interface (`ui-version.json`): quando o build foi gerado (UTC, sem milissegundos) e o
 * commit. O Client compara com a interface publicada sem instalador (src-tauri/src/interface.rs), e
 * scripts/publicar-interface.ps1 põe os mesmos valores no manifesto.
 */
function uiVersion(): Plugin {
  return {
    name: "shmupx-ui-version",
    apply: "build",
    generateBundle() {
      const built = process.env.SHMUPX_UI_BUILT || new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
      let commit = "";
      try {
        commit = execSync("git rev-parse --short HEAD", { encoding: "utf8" }).trim();
      } catch {
        // fora de um repositório git
      }
      this.emitFile({ type: "asset", fileName: "ui-version.json", source: JSON.stringify({ built, commit }) });
    },
  };
}

// Tauri espera uma porta fixa em dev.
export default defineConfig({
  plugins: [react(), tailwindcss(), uiVersion()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
});
