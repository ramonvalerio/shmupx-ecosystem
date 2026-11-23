import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";

/** Espelha `src-tauri/src/validacao/dados.rs`. */
export type VideoOrigin = { tipo: "youtube"; url: string } | { tipo: "arquivo" };

export type VideoState =
  | { fase: "baixando"; progresso: number; sessao: number }
  | { fase: "pronto" }
  | { fase: "na_fila"; sessao: number; ordem: number }
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
  /** Jogadores na partida (2 = cooperativa); `null` em resultados antigos. */
  jogadores: number | null;
  /** A volta foi completada (jogo zerado); `null` em resultados antigos. */
  zerado: boolean | null;
  /** Bombas gastas, quando o script do jogo conta bombas. */
  bombas: number | null;
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

/** Uma versão do script de um jogo (`validators/games/<id>/<version>/`). */
export interface ScriptInfo {
  id: string;
  name: string;
  version: string;
  status: "draft" | "approved";
  /** Imagens e arquivos que o script pede e ainda não existem; vazio = pronto para rodar. */
  missing: string[];
}

function versionKey(version: string): number[] {
  return version.split(".").map(Number);
}

function compareVersions(a: string, b: string): number {
  const [x, y] = [versionKey(a), versionKey(b)];
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] - y[i];
  return 0;
}

/** Versões do script do jogo, da mais nova para a mais antiga. */
export function scriptsOf(scripts: ScriptInfo[], gameId: string): ScriptInfo[] {
  return scripts.filter((script) => script.id === gameId).sort((a, b) => compareVersions(b.version, a.version));
}

/** A versão que vale por padrão (a mesma regra do motor): a mais nova aprovada, senão a mais nova. */
export function defaultScript(versions: ScriptInfo[]): ScriptInfo | null {
  return versions.find((script) => script.status === "approved") ?? versions[0] ?? null;
}

export interface ValidationEnvironment {
  python: EnvironmentItem;
  dependencias: EnvironmentItem;
  ytdlp: EnvironmentItem;
  /** Deno, o runtime JavaScript que o yt-dlp usa para o YouTube. */
  deno: EnvironmentItem;
  scripts: ScriptInfo[];
  /** Navegador de onde o yt-dlp lê os cookies do YouTube; `null` = sem cookies. */
  cookies_navegador: string | null;
}

/** Navegadores que o `--cookies-from-browser` do yt-dlp conhece (mesma lista de `ytdlp.rs`). */
export const COOKIE_BROWSERS: { id: string; label: string }[] = [
  { id: "firefox", label: "Firefox" },
  { id: "chrome", label: "Chrome" },
  { id: "edge", label: "Edge" },
  { id: "brave", label: "Brave" },
  { id: "opera", label: "Opera" },
  { id: "vivaldi", label: "Vivaldi" },
  { id: "chromium", label: "Chromium" },
  { id: "safari", label: "Safari" },
];

/** Escolhe o navegador dos cookies do YouTube (`null` desliga). Vale para os próximos downloads. */
export const setCookiesBrowser = (navegador: string | null) => invoke<void>("validacao_definir_cookies", { navegador });

/** O YouTube pediu login para confirmar que não é um robô (inclui mensagens antigas, cruas do yt-dlp). */
export function isYoutubeBlock(error: string): boolean {
  return error.startsWith("O YouTube bloqueou") || error.includes("not a bot");
}

export const getEnvironment = () => invoke<ValidationEnvironment>("validacao_ambiente");

/** Instala (ou atualiza) o yt-dlp oficial na pasta do Lab, conferindo o SHA-256 da release. */
export const installYtDlp = () => invoke<EnvironmentItem>("validacao_instalar_ytdlp");

/** Arquivo do validador de um jogo, para leitura na tela. */
export interface ScriptSourceFile {
  nome: string;
  caminho: string;
  linguagem: "python" | "json" | "markdown";
  conteudo: string;
  truncado: boolean;
}

/** Manifesto, calibração, passo a passo e analisador de uma versão do script (só leitura). */
export const getScriptSource = (gameId: string, versao: string) => invoke<ScriptSourceFile[]>("validacao_script_fonte", { gameId, versao });

/** Resultado de "Gerar script" e "Nova versão": pasta criada e recortes do HUD que faltam. */
export interface GeneratedScript {
  versao: string;
  criados: string[];
  imagens_pendentes: string[];
}

/** Cria o primeiro script do jogo (0.1.0, rascunho) a partir do Earthion. Não sobrescreve arquivos. */
export const generateScript = (gameId: string, titulo: string) => invoke<GeneratedScript>("validacao_gerar_script", { gameId, titulo });

/** Cria a próxima versão (rascunho) copiando calibração e imagens de `deVersao`. */
export const newScriptVersion = (gameId: string, deVersao: string) => invoke<GeneratedScript>("validacao_nova_versao", { gameId, deVersao });

/** Versão aprovada; `cliente_atualizado`: a lista de scripts do Client (ao lado, no repositório) já a aceita. */
export interface ApprovedScript {
  versao: string;
  cliente_atualizado: boolean;
}

/** Aprova uma versão do script (rascunho → aprovado) e a põe na lista do Client. */
export const approveScriptVersion = (gameId: string, versao: string) => invoke<ApprovedScript>("validacao_aprovar_versao", { gameId, versao });

export const listVideos = (gameId: string) => invoke<ValidationDataset>("validacao_listar", { gameId });

export const addYoutubeVideo = (gameId: string, url: string) =>
  invoke<ValidationVideo>("validacao_adicionar_youtube", { gameId, url });

export const addLocalVideos = (gameId: string, caminhos: string[]) =>
  invoke<number>("validacao_adicionar_arquivos", { gameId, caminhos });

export const setExpected = (gameId: string, videoId: string, gabarito: Expected) =>
  invoke<void>("validacao_definir_gabarito", { gameId, videoId, gabarito });

export const removeVideo = (gameId: string, videoId: string) => invoke<void>("validacao_remover", { gameId, videoId });

/** Põe os vídeos na fila de análise com a versão `versao` do script. */
export const runVideos = (gameId: string, versao: string, videoIds: string[]) => invoke<number>("validacao_rodar", { gameId, versao, videoIds });

/** Tira da fila, ou interrompe o download/análise em andamento. */
export const cancelProcessing = (gameId: string, videoId: string) => invoke<void>("validacao_cancelar", { gameId, videoId });

/** Tenta de novo o download de um vídeo do YouTube que falhou ou foi cancelado. */
export const downloadAgain = (gameId: string, videoId: string) => invoke<void>("validacao_baixar_de_novo", { gameId, videoId });

/** Abre o link no navegador padrão do sistema. */
export const openInBrowser = (url: string) => openUrl(url);

/** Posição na fila de análise (1 = o próximo a rodar), ou `null` se o vídeo não está na fila. */
export function queuePosition(videos: ValidationVideo[], video: ValidationVideo): number | null {
  if (video.estado.fase !== "na_fila") return null;
  const mine = video.estado.ordem;
  return videos.filter((other) => other.estado.fase === "na_fila" && other.estado.ordem < mine).length + 1;
}

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
