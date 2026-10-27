import { invoke } from "@tauri-apps/api/core";

/** Proposta da IA para um jogo. É só uma sugestão: o admin revisa antes de salvar. */
export interface GameSuggestion {
  name: string;
  developer: string | null;
  publisher: string | null;
  genre: string | null;
  year: number | null;
  platform: string | null;
  /** Palpite do modelo, não um fato: deve ser conferido na loja da Steam. */
  steam_app_id: number | null;
  modes: string[];
}

export interface SuggestionResult {
  sugestoes: GameSuggestion[];
  /** `true` quando veio do cache local: nenhuma chamada à API foi feita nem cobrada. */
  do_cache: boolean;
  modelo: string | null;
  /** A loja da Steam foi consultada e conferiu o resultado: um `steam_app_id` presente é confirmado. */
  steam_consultada: boolean;
  jogos_na_steam: number;
  /** Avisos para mostrar ao admin (ex.: a Steam não respondeu e a IA foi usada sozinha). */
  avisos: string[];
}

export interface SuggestionOptions {
  /** Ancora a resposta numa pesquisa na internet (mais lento e mais caro). */
  web: boolean;
  /** Consulta a loja da Steam (gratuito) e usa os dados dela como fonte confiável. */
  steam: boolean;
  /** Força uma consulta nova, sem usar o cache local. */
  ignoreCache: boolean;
}

/**
 * Pede ao Rust do Lab sugestões de metadados para o título. A chave da API fica no ambiente do
 * PC e nunca passa por aqui.
 */
export function suggestGame(title: string, options: SuggestionOptions): Promise<SuggestionResult> {
  return invoke<SuggestionResult>("sugerir_jogo", {
    titulo: title,
    web: options.web,
    steam: options.steam,
    ignorarCache: options.ignoreCache,
  });
}
