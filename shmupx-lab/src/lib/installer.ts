import { invoke } from "@tauri-apps/api/core";
import type { ReleaseInstaller } from "./backend";

/** Build local do Client para a versão (`shmupx-client/src-tauri/target/release/bundle/nsis`). */
export interface LocalInstaller {
  found: boolean;
  /** Existe o `.sig` do updater ao lado do `.exe`. */
  signed: boolean;
  file: string;
  size?: number;
  built_at_ms?: number;
}

export const getLocalInstaller = (versao: string) => invoke<LocalInstaller>("instalador_local", { versao });

/** Envia o instalador assinado ao S3 (credenciais do OpenBao) e devolve URL e assinatura. */
export const publishInstaller = (versao: string) => invoke<Required<ReleaseInstaller>>("publicar_instalador", { versao });

export function formatSize(bytes: number | undefined): string {
  if (!bytes) return "—";
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
