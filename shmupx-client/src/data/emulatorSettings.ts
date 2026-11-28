import { useCallback, useState } from "react";
import { readMisterLibrary } from "./misterLibrary";
import { misterAuth } from "./misterPlayer";
import { invoke, isTauri } from "@tauri-apps/api/core";

/** Como o ShmupX Engine mostra o jogo no PC (Configurações › Emulador). Mesmos campos do Rust (`VideoDoMotor`). */
export interface EmulatorSettings {
  /** Abre em janela; desmarcado, em tela cheia. */
  windowed: boolean;
  /** Em tela cheia: sem borda (borderless), em vez da exclusiva. */
  borderless: boolean;
  /** Linhas de varredura de TV de tubo por cima da imagem. */
  scanlines: boolean;
  /**
   * `native`: resolução do monitor do PC; `crt15`: monitor CRT de arcade de 15 kHz (240p) ligado no
   * PC; `crt31`: monitor CRT de PC de 31 kHz (VGA) ligado no PC; `mister`: monitor CRT ligado no
   * MiSTer (sem GPU, a imagem vai pela rede). O MiSTer é escolhido em Monitor dos jogos.
   */
  resolution: EmulatorResolution;
  /** Monitor onde os jogos abrem, pelo nome do Windows (`\\.\DISPLAY2`); vazio = o principal do Windows. */
  monitor: string;
  /** Play on MiSTer: no monitor CRT do MiSTer, joga com o controle ligado no MiSTer; desmarcado, com o deste PC. */
  playOnMister: boolean;
}

/** Monitor ligado ao PC (Rust `MonitorDoPc`). */
export interface PcMonitor {
  id: string;
  numero: number;
  largura: number;
  altura: number;
  principal: boolean;
}

/** Monitores ligados ao PC, do principal para os outros (só no app desktop). */
export async function listMonitors(): Promise<PcMonitor[]> {
  if (!isTauri()) return [];
  return invoke<PcMonitor[]>("emulador_monitores");
}

/**
 * Mostra o número do monitor, grande e em vermelho, no meio dele por 3 segundos (ao escolher o
 * monitor dos jogos). `id` vazio = o principal do Windows. Fora do app desktop, não faz nada.
 */
export function identifyMonitor(id: string) {
  if (!isTauri()) return;
  invoke("emulador_identificar_monitor", { id }).catch(() => undefined);
}

export type EmulatorResolution = "native" | "crt15" | "crt31" | "mister";

/** Modo de tela dos jogos: em janela, tela cheia (exclusiva) ou tela cheia sem borda (borderless). */
export type ScreenMode = "window" | "fullscreen" | "borderless";

export const SCREEN_MODES: { id: ScreenMode; label: string; detail: string }[] = [
  { id: "window", label: "Janela", detail: "Ao lado do Client e do chat" },
  { id: "fullscreen", label: "Tela cheia", detail: "O jogo ocupa o monitor inteiro" },
  { id: "borderless", label: "Tela cheia sem borda", detail: "Alt+Tab sem piscar a tela" },
];

/**
 * Os modos que o jogador escolhe. A tela cheia exclusiva (`fullscreen`), que prende a tela, está
 * fora por enquanto: no monitor do PC a tela cheia é sempre sem borda (também ao passar da janela
 * para a tela cheia no jogo, com duplo clique ou Alt+Enter).
 */
export const SCREEN_MODE_CHOICES = SCREEN_MODES.filter((mode) => mode.id !== "fullscreen");

/** Modo de tela das preferências; a tela cheia exclusiva salva antes vale como sem borda. */
export function screenMode(settings: EmulatorSettings): ScreenMode {
  return settings.windowed ? "window" : "borderless";
}

/** As preferências com o modo de tela escolhido (em janela, mantém a escolha de sem borda). */
export function withScreenMode(settings: EmulatorSettings, mode: ScreenMode): EmulatorSettings {
  return mode === "window"
    ? { ...settings, windowed: true, scanlines: true }
    : { ...settings, windowed: false, borderless: mode === "borderless", scanlines: true };
}

/** Resoluções de um monitor ligado no PC (Vídeo do jogo › Resolução). */
export const EMULATOR_RESOLUTIONS: { id: EmulatorResolution; label: string; tile: string; detail: string }[] = [
  { id: "native", label: "Monitor de PC moderno", tile: "Recomendado", detail: "Usa a resolução atual do seu monitor, sem trocar o modo de vídeo." },
  { id: "crt31", label: "Monitor CRT de PC 31 kHz", tile: "480p, para monitor de tubo de PC", detail: "Para PC ligado num monitor de tubo de PC (VGA). O vídeo troca para o modo do jogo, em 31 kHz." },
  { id: "crt15", label: "Monitor CRT de arcade 15 kHz", tile: "240p, para monitor de arcade", detail: "Para PC ligado direto numa TV de tubo ou monitor de arcade. As scanlines ficam por conta da TV." },
];

/** O monitor CRT ligado no MiSTer (Vídeo do jogo › Monitor dos jogos): o jogo roda neste PC, sem a GPU. */
export const MISTER_MONITOR = {
  label: "Monitor CRT no MiSTer (sem GPU)",
  tile: "Vídeo enviado pelo cabo de rede. A GPU do PC não é usada.",
  detail: "O controle é o deste PC ou, com Play on MiSTer marcado em Controle, o ligado no MiSTer.",
};

const KEY = "shmupx.emulator.settings";

export function defaultEmulatorSettings(): EmulatorSettings {
  return { windowed: true, borderless: false, scanlines: true, resolution: "native", monitor: "", playOnMister: false };
}

/** Preferências salvas neste computador; sem armazenamento, valem as padrão. */
export function readEmulatorSettings(): EmulatorSettings {
  const fallback = defaultEmulatorSettings();
  try {
    const saved = JSON.parse(window.localStorage.getItem(KEY) ?? "null") as Partial<EmulatorSettings> | null;
    if (!saved || typeof saved !== "object") return fallback;
    return {
      windowed: typeof saved.windowed === "boolean" ? saved.windowed : fallback.windowed,
      borderless: typeof saved.borderless === "boolean" ? saved.borderless : fallback.borderless,
      // Sem opção na tela por enquanto: acima de 240p (fora da TV de tubo) as scanlines ficam sempre ligadas.
      scanlines: true,
      resolution: saved.resolution === "mister" || EMULATOR_RESOLUTIONS.some((r) => r.id === saved.resolution) ? (saved.resolution as EmulatorResolution) : fallback.resolution,
      monitor: typeof saved.monitor === "string" ? saved.monitor : fallback.monitor,
      playOnMister: typeof saved.playOnMister === "boolean" ? saved.playOnMister : fallback.playOnMister,
    };
  } catch {
    return fallback;
  }
}

/** Preferências do emulador com gravação automática neste computador. `saved` = false se o armazenamento falhou. */
export function useEmulatorSettings() {
  const [settings, setSettings] = useState<EmulatorSettings>(readEmulatorSettings);
  const [saved, setSaved] = useState(true);
  const update = useCallback((next: EmulatorSettings) => {
    setSettings(next);
    try {
      window.localStorage.setItem(KEY, JSON.stringify(next));
      setSaved(true);
    } catch {
      setSaved(false);
    }
  }, []);
  return { settings, update, saved };
}

/**
 * Preferências de vídeo para abrir o jogo (Rust `VideoDoMotor`): no monitor CRT do MiSTer, com o
 * endereço do MiSTer conectado no Client e o acesso SSH, para o Client abrir o core Groovy nele.
 */
export function emulatorVideoForLaunch(): EmulatorSettings & { misterHost?: string; misterUsuario?: string | null; misterSenha?: string | null } {
  const settings = readEmulatorSettings();
  if (settings.resolution !== "mister") return settings;
  const host = readMisterLibrary()?.host ?? "";
  const { usuario, senha } = misterAuth();
  // Play on MiSTer só com o MiSTer conectado na importação.
  return { ...settings, playOnMister: Boolean(host) && settings.playOnMister, misterHost: host, misterUsuario: usuario, misterSenha: senha };
}
