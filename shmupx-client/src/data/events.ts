import { useCallback, useEffect, useSyncExternalStore } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { GameMode, GameRoom } from "../types/room";

/**
 * Eventos do Lab no Client (backend `GET /api/events`): os publicados que ainda não terminaram, com
 * a inscrição do jogador. A inscrição paga as fichas do evento; inscrito, o jogador joga partidas
 * valendo no evento pela ficha (Event Mode, ao lado do Training e do Ranked).
 */
export interface ClientEvent {
  id: string;
  title: string;
  description: string;
  game_ids: string[];
  /** Nomes dos jogos do evento, na ordem de `game_ids`. */
  game_titles: string[];
  /** Tabela do ranking que vale: `highscore`, `1cc` ou `caravan`. */
  mode: GameMode;
  starts_at: string | null;
  ends_at: string | null;
  rules: string[];
  prizes: string[];
  /** Fichas para se inscrever (0 = grátis). */
  entry_fee: number;
  banner_url: string;
  page_url: string;
  joined: boolean;
}

interface EventsState { events: ClientEvent[]; loading: boolean; error: string | null; loadedAt: number }

/** De quanto em quanto tempo a lista é lida de novo (eventos novos, começo e fim). */
const REFRESH_MS = 5 * 60_000;
let state: EventsState = { events: [], loading: false, error: null, loadedAt: 0 };
const listeners = new Set<() => void>();
let inFlight: Promise<void> | null = null;

function set(next: Partial<EventsState>) {
  state = { ...state, ...next };
  listeners.forEach((listener) => listener());
}

const describe = (error: unknown) => (error instanceof Error ? error.message : String(error));

/** Lê de novo os eventos (uma leitura de cada vez). */
export function reloadEvents(): Promise<void> {
  if (!isTauri()) return Promise.resolve();
  if (inFlight) return inFlight;
  set({ loading: true });
  inFlight = invoke<ClientEvent[]>("eventos_listar")
    .then((events) => set({ events, error: null, loadedAt: Date.now() }))
    .catch((error) => set({ error: describe(error) }))
    .finally(() => { set({ loading: false }); inFlight = null; });
  return inFlight;
}

/** Inscreve o jogador (paga as fichas do evento). Devolve o evento atualizado. */
export async function joinEvent(id: string): Promise<ClientEvent> {
  const result = await invoke<{ event: ClientEvent; balance: number | null }>("evento_participar", { id });
  set({ events: state.events.map((e) => (e.id === id ? result.event : e)) });
  return result.event;
}

/** Abre a página do evento no navegador. */
export function openEventPage(url: string): Promise<void> {
  return invoke("evento_abrir_pagina", { url });
}

/** Lista de eventos, compartilhada por todas as telas; carrega no primeiro uso e a cada 5 min. */
export function useEvents(enabled: boolean) {
  const current = useSyncExternalStore((listener) => { listeners.add(listener); return () => listeners.delete(listener); }, () => state);
  useEffect(() => {
    if (!enabled) return;
    if (Date.now() - state.loadedAt > REFRESH_MS) void reloadEvents();
    const timer = window.setInterval(() => void reloadEvents(), REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [enabled]);
  const join = useCallback((id: string) => joinEvent(id), []);
  return { ...current, reload: reloadEvents, join };
}

/** O evento está acontecendo agora (entre o começo e o fim). */
export function eventRunning(event: ClientEvent, now = Date.now()): boolean {
  const start = event.starts_at ? Date.parse(event.starts_at) : 0;
  const end = event.ends_at ? Date.parse(event.ends_at) : 0;
  return now >= start && now < end;
}

/** Eventos em que o jogador pode jogar valendo nesta sala: inscrito e o jogo é do evento. */
export function playableEvents(events: ClientEvent[], room: GameRoom): ClientEvent[] {
  return events.filter((e) => e.joined && e.game_ids.includes(room.id));
}

/** Ordem do carrossel: os do jogo da sala primeiro, depois os outros, cada grupo pela data de começo. */
export function carouselOrder(events: ClientEvent[], room: GameRoom): ClientEvent[] {
  const here = (e: ClientEvent) => (e.game_ids.includes(room.id) ? 0 : 1);
  return [...events].sort((a, b) => here(a) - here(b) || (a.starts_at ?? "").localeCompare(b.starts_at ?? ""));
}
