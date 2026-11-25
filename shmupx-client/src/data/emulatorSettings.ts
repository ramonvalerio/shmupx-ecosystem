import { useCallback, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";

/** Como o ShmupX Engine mostra o jogo no PC (Configurações › Emulador). Mesmos campos do Rust (`VideoDoMotor`). */
export interface EmulatorSettings {
  /** Abre em janela; desmarcado, em tela cheia. */
  windowed: boolean;
  /** Linhas de varredura de TV de tubo por cima da imagem. */
  scanlines: boolean;
  /** `native`: resolução do monitor do PC; `crt15`: TV de tubo de 15 kHz (240p) ligada no PC. */
  resolution: EmulatorResolution;
  /** Monitor onde os jogos abrem, pelo nome do Windows (`\\.\DISPLAY2`); vazio = o principal do Windows. */
  monitor: string;
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

export type EmulatorResolution = "native" | "crt15";

export const EMULATOR_RESOLUTIONS: { id: EmulatorResolution; label: string; detail: string }[] = [
  { id: "native", label: "Resolução do monitor (recomendado)", detail: "Usa a resolução atual do seu monitor, sem trocar o modo de vídeo." },
  { id: "crt15", label: "TV de tubo 15 kHz (240p)", detail: "Para PC ligado direto numa TV de tubo ou monitor de arcade. As scanlines ficam por conta da TV." },
];

const KEY = "shmupx.emulator.settings";

export function defaultEmulatorSettings(): EmulatorSettings {
  return { windowed: true, scanlines: true, resolution: "native", monitor: "" };
}

/** Preferências salvas neste computador; sem armazenamento, valem as padrão. */
export function readEmulatorSettings(): EmulatorSettings {
  const fallback = defaultEmulatorSettings();
  try {
    const saved = JSON.parse(window.localStorage.getItem(KEY) ?? "null") as Partial<EmulatorSettings> | null;
    if (!saved || typeof saved !== "object") return fallback;
    return {
      windowed: typeof saved.windowed === "boolean" ? saved.windowed : fallback.windowed,
      // Sem opção na tela por enquanto: acima de 240p (fora da TV de tubo) as scanlines ficam sempre ligadas.
      scanlines: true,
      resolution: EMULATOR_RESOLUTIONS.some((r) => r.id === saved.resolution) ? (saved.resolution as EmulatorResolution) : fallback.resolution,
      monitor: typeof saved.monitor === "string" ? saved.monitor : fallback.monitor,
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
