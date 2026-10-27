import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";

/** Espelha `src-tauri/src/validacao/dados.rs`. */
export type VideoOrigin = { tipo: "youtube"; url: string } | { tipo: "arquivo" };

export type VideoState =
  | { fase: "baixando"; progresso: number; sessao: number }
  | { fase: "pronto" }
  | { fase: "na_fila"; sessao: number }
  | { fase: "analisando"; progresso: number; sessao: number }
  | { fase: "concluido" }
  | { fase: "falhou"; erro: string };

/** Valores reais da gameplay (gabarito). Vidas e continues são opcionais. */
export interface Expected {
  placar: number | null;
  vidas: number | null;
  continues: number | null;
}

export interface ScriptResult {
  placar: number | null;
  vidas: number | null;
  continues: number | null;
  game_over: boolean;
  confianca: string;
  revisao: boolean;
  observacao: string | null;
  versao_validador: string | null;
  /** O Client aceitaria esta run sem revisão. */
  aceito_automatico: boolean;
  /** O script falhou (processo com erro ou relatório inválido). */
  erro: string | null;
  relatorio: unknown;
  em: number;
  duracao_segundos: number;
}

export interface ValidationVideo {
  id: string;
  titulo: string;
  origem: VideoOrigin;
  caminho: string | null;
  adicionado_em: number;
  /** Duração em segundos, quando conhecida (metadados do YouTube ou leitura do arquivo). */
  duracao_segundos: number | null;
  gabarito: Expected;
  estado: VideoState;
  resultado: ScriptResult | null;
  /** Placar lido = gabarito (e vidas/continues, se informados). `null` sem gabarito ou análise. */
  acertou: boolean | null;
}

export interface ValidationSummary {
  total: number;
  analisados: number;
  com_gabarito: number;
  acertos: number;
  aceitos: number;
}

export interface ValidationDataset {
  videos: ValidationVideo[];
  resumo: ValidationSummary;
}

export interface EnvironmentItem {
  ok: boolean;
  detalhe: string;
}

export interface ScriptInfo {
  id: string;
  name: string;
  version: string;
}

export interface ValidationEnvironment {
  python: EnvironmentItem;
  dependencias: EnvironmentItem;
  ytdlp: EnvironmentItem;
  scripts: ScriptInfo[];
}

export const getEnvironment = () => invoke<ValidationEnvironment>("validacao_ambiente");

/** Instala (ou atualiza) o yt-dlp oficial na pasta do Lab, conferindo o SHA-256 da release. */
export const installYtDlp = () => invoke<EnvironmentItem>("validacao_instalar_ytdlp");

export const listVideos = (gameId: string) => invoke<ValidationDataset>("validacao_listar", { gameId });

export const addYoutubeVideo = (gameId: string, url: string) =>
  invoke<ValidationVideo>("validacao_adicionar_youtube", { gameId, url });

export const addLocalVideos = (gameId: string, caminhos: string[]) =>
  invoke<number>("validacao_adicionar_arquivos", { gameId, caminhos });

export const setExpected = (gameId: string, videoId: string, gabarito: Expected) =>
  invoke<void>("validacao_definir_gabarito", { gameId, videoId, gabarito });

export const removeVideo = (gameId: string, videoId: string) => invoke<void>("validacao_remover", { gameId, videoId });

export const runVideos = (gameId: string, videoIds: string[]) => invoke<number>("validacao_rodar", { gameId, videoIds });

/** Tira da fila, ou interrompe o download/análise em andamento. */
export const cancelProcessing = (gameId: string, videoId: string) => invoke<void>("validacao_cancelar", { gameId, videoId });

/** Tenta de novo o download de um vídeo do YouTube que falhou ou foi cancelado. */
export const downloadAgain = (gameId: string, videoId: string) => invoke<void>("validacao_baixar_de_novo", { gameId, videoId });

/** Abre o link no navegador padrão do sistema. */
export const openInBrowser = (url: string) => openUrl(url);

/** Há algo em andamento no vídeo (download, fila ou análise). */
export function isBusy(video: ValidationVideo): boolean {
  return video.estado.fase === "baixando" || video.estado.fase === "na_fila" || video.estado.fase === "analisando";
}

/** Download que falhou e pode ser tentado de novo. */
export function canDownloadAgain(video: ValidationVideo): boolean {
  return video.estado.fase === "falhou" && video.origem.tipo === "youtube";
}

/** 45_000 -> "~1 min"; 3_900_000 -> "~1 h 05 min". */
export function formatRemaining(ms: number): string {
  const minutes = Math.round(ms / 60_000);
  if (minutes < 1) return "menos de 1 min";
  if (minutes < 60) return `~${minutes} min`;
  return `~${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, "0")} min`;
}

/** Avisa quando o banco de um jogo muda (download, fila, análise). */
export function onDatasetChanged(callback: (gameId: string) => void): Promise<UnlistenFn> {
  return listen<{ game_id: string }>("validacao://atualizado", (event) => callback(event.payload.game_id));
}

/** Percentual com uma casa, ou `null` sem base. */
export function percent(part: number, total: number): string | null {
  return total > 0 ? `${((part / total) * 100).toFixed(1).replace(".", ",")}%` : null;
}

/** 2518 -> "41:58"; 3725 -> "1:02:05". */
export function formatDuration(seconds: number | null | undefined): string | null {
  if (seconds === null || seconds === undefined) return null;
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

export function formatScore(value: number | null | undefined): string {
  return value === null || value === undefined ? "—" : value.toLocaleString("pt-BR");
}
