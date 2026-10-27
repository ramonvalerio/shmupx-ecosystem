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
}

/**
 * Pede ao Rust do Lab sugestões de metadados para o título (`web` ancora numa pesquisa na
 * internet; `ignoreCache` força consulta nova). A chave da API fica no ambiente do PC e nunca
 * passa por aqui.
 */
export function suggestGame(title: string, web: boolean, ignoreCache: boolean): Promise<SuggestionResult> {
  return invoke<SuggestionResult>("sugerir_jogo", { titulo: title, web, ignorarCache: ignoreCache });
}
