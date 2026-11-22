import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// O Lite reaproveita do Client (shmupx-client/src) o login, o catálogo, o chat e a presença:
// `@client/...`. As bibliotecas vêm sempre deste projeto (uma só cópia do React e do Firebase).
const client = fileURLToPath(new URL("../shmupx-client/src", import.meta.url));
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@client": client },
    dedupe: ["react", "react-dom", "firebase", "@firebase/app", "@firebase/auth", "@firebase/firestore", "@tauri-apps/api"],
  },
  // Imagens do Client (logo, avatar, capas curadas).
  publicDir: fileURLToPath(new URL("../shmupx-client/public", import.meta.url)),
  clearScreen: false,
  server: {
    port: 1430,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1431 } : undefined,
    fs: { allow: [".."] },
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
