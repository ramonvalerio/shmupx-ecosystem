import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
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

/**
 * Gera o instalador assinado da versão no shmupx-client (`npm run bundle`): ajusta a versão do
 * Client e usa a configuração e a chave do updater do OpenBao. O andamento chega em `onInstallerBuild`.
 */
export const buildInstaller = (versao: string) => invoke<void>("gerar_instalador", { versao });

export interface InstallerBuildEvent {
  versao: string;
  fase: "rodando" | "pronto" | "falhou";
  linha?: string | null;
  erro?: string | null;
}

export const onInstallerBuild = (callback: (event: InstallerBuildEvent) => void): Promise<UnlistenFn> =>
  listen<InstallerBuildEvent>("instalador://build", (event) => callback(event.payload));

export function formatSize(bytes: number | undefined): string {
  if (!bytes) return "—";
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

/** Novidades/correções do Client lidas dos commits desde a versão anterior (git local). */
export interface ClientNotes {
  features: string[];
  fixes: string[];
  since?: string | null;
  commits: number;
  rewritten: boolean;
  rewrite_error?: string | null;
}
export const getClientNotes = (versao: string) => invoke<ClientNotes>("notas_do_client", { versaoAlvo: versao });
