import pkg from "../package.json";

/** Versão do app (a mesma do APK: package.json, tauri.conf.json e Cargo.toml sobem juntos). */
export const APP_VERSION: string = pkg.version;
