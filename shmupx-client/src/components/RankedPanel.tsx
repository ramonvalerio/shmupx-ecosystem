import { useId, useState, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import type { GameMode, GameRoom, PlayOn } from "../types/room";
import { liveActive, useLive, useYouTube } from "../data/useLive";
import { readEmulatorLibrary, romsFor } from "../data/emulatorLibrary";
import { isSupported } from "../data/validation";
import CreditsBadge from "./CreditsBadge";
import type { useGameLauncher } from "./useGameLauncher";

/** Opções do Play Ranked: transmitir no YouTube, se a live é pública e tela cheia na gravação. */
export interface LivePrefs { on: boolean; public: boolean; fullscreen: boolean }

// Lembradas entre as salas e a Coleção (as duas telas mostram e mudam as mesmas opções).
const KEY = "shmupx.client.livePrefs.v1";
const listeners = new Set<() => void>();
let cache: { raw: string | null; value: LivePrefs } | null = null;
function readLivePrefs(): LivePrefs {
  let raw: string | null = null;
  try { raw = localStorage.getItem(KEY); } catch { /* sem armazenamento: padrões */ }
  if (cache && cache.raw === raw) return cache.value;
  let value: LivePrefs = { on: false, public: false, fullscreen: true };
  try {
    const saved = JSON.parse(raw ?? "{}") as Partial<LivePrefs>;
    value = { on: saved.on === true, public: saved.public === true, fullscreen: saved.fullscreen !== false };
  } catch { /* valor inválido: padrões */ }
  cache = { raw, value };
  return value;
}
export function setLivePrefs(next: Partial<LivePrefs>) {
  try { localStorage.setItem(KEY, JSON.stringify({ ...readLivePrefs(), ...next })); } catch { /* fica só nesta sessão */ }
  listeners.forEach((listener) => listener());
}
export function useLivePrefs(): LivePrefs {
  return useSyncExternalStore((listener) => { listeners.add(listener); return () => listeners.delete(listener); }, readLivePrefs);
}

/** Live do jogo: começar (abrindo o jogo na plataforma escolhida) e encerrar. */
export function useRankedLive(room: GameRoom) {
  const live = useLive();
  const prefs = useLivePrefs();
  const [liveError, setLiveError] = useState<string | null>(null);
  const liveHere = live.state.game_id === room.id;
  const liveOn = liveActive(live.state);

  /** Play Ranked com live: na Steam a live abre o jogo sozinha; o emulador abre antes. */
  async function start(platform: PlayOn, ensureInstalled: (platform: PlayOn) => Promise<boolean>) {
    setLiveError(null);
    if (!(await ensureInstalled(platform))) return;
    try {
      if (platform === "emulator") {
        await invoke("inicializar_jogo_emulador", { jogoId: room.id, roms: romsFor(room), pasta: readEmulatorLibrary()?.root || null, treino: true, janela: !prefs.fullscreen });
        await live.start(room.id, `${room.title} · ShmupX`, prefs.public, 0);
      } else {
        await live.start(room.id, `${room.title} · ShmupX`, prefs.public, room.steamAppId);
      }
    } catch (error) {
      setLiveError(error instanceof Error ? error.message : String(error));
    }
  }

  async function stop() {
    setLiveError(null);
    try {
      await live.stop();
    } catch (error) {
      setLiveError(error instanceof Error ? error.message : String(error));
    }
  }

  return { live, prefs, liveError, setLiveError, liveHere, liveOn, start, stop };
}

/**
 * Play Ranked: a ficha (clique nela para jogar valendo ranking; ela gira) com o saldo e, ao lado
 * ou abaixo, as opções da partida — live no YouTube, quem pode assistir e tela cheia na gravação.
 * Usado na sala (cabeçalho) e no painel da Coleção (`compact`).
 */
export default function RankedPanel({ room, uid, launcher, mode, platform, ranked, busy, compact = false }: {
  room: GameRoom;
  uid: string | undefined;
  launcher: ReturnType<typeof useGameLauncher>;
  mode: GameMode;
  platform: PlayOn | null;
  ranked: ReturnType<typeof useRankedLive>;
  /** Jogo aberto ou parando: sem Play Ranked e com as opções presas. */
  busy: boolean;
  compact?: boolean;
}) {
  const { t } = useI18n();
  const uidLabel = useId();
  const youtube = useYouTube();
  // A cada Play Ranked, a moeda das fichas é lançada e gira.
  const [coinSpin, setCoinSpin] = useState(0);
  const { live, prefs, liveHere, liveOn } = ranked;
  const comingSoon = !isSupported(room);
  // Live indisponível: no MiSTer (a live transmite a janela do jogo neste PC) ou sem canal conectado.
  const liveUnavailable = platform === "mister"
    ? t("No MiSTer não dá para transmitir: a live mostra a janela do jogo neste computador.")
    : !youtube.connection?.connected ? t("Para transmitir, conecte seu canal do YouTube em Configurações › Integrações.") : null;
  const withLive = prefs.on && !liveUnavailable;
  // Tela cheia: o Client controla só o emulador (FBNeo); na Steam vale a configuração do jogo.
  const fullscreenNote = platform === "steam" ? t("Na Steam, o jogo abre com a configuração de tela dele.")
    : platform === "mister" ? t("No MiSTer, o jogo aparece na TV.")
    : t("Desligada, o jogo abre em janela.");
  const liveLabel = live.state.status === "waiting" ? t("Abrindo o jogo…")
    : live.state.status === "preparing" ? t("Criando a live…")
    : live.state.status === "connecting" ? t("Conectando ao YouTube…")
    : live.state.status === "ending" ? t("Encerrando…")
    : t("End live");
  const optionsLocked = comingSoon || busy || liveOn;
  // Live deste jogo no ar (ou abrindo): as opções mostram como ela está.
  const liveHereOn = liveOn && liveHere;

  /** Com Live On, abre o jogo já transmitindo; sem, valendo ranking. */
  function playRanked() {
    setCoinSpin((n) => n + 1);
    ranked.setLiveError(null);
    const kind = withLive ? "live" : "ranked";
    if (platform && (kind === "ranked" || platform !== "mister")) launcher.playNow(kind, platform, mode);
    else launcher.play(kind);
  }

  return (
    <div className={`room-header-side ${compact ? "is-compact" : ""}`}>
      <CreditsBadge uid={uid} className="room-credits" spin={coinSpin} onPlay={playRanked} live={withLive}
        playDisabled={launcher.isLaunching || comingSoon || busy || liveOn}
        playTitle={comingSoon ? t("Coming soon: Play Ranked fica disponível quando o jogo tiver o script de validação.") : liveOn && !liveHere ? t("Há uma live em andamento em outra sala.") : undefined} />
      <div className="room-coin-actions">
        {liveHereOn && (
          <button type="button" className="room-action live-action is-on" onClick={() => void ranked.stop()} disabled={live.state.status === "ending"}>
            <span className="action-icon is-square" aria-hidden="true" />{liveLabel}
          </button>
        )}
        {/* Como a partida valendo ranking vai acontecer: com ou sem live, e quem pode assistir. */}
        <div className="ranked-options" role="group" aria-label={t("Opções do Play Ranked")}>
          <div className="room-setup-group">
            <span id={`${uidLabel}-live`}>{t("Live no YouTube")}</span>
            <div className="room-chips" role="radiogroup" aria-labelledby={`${uidLabel}-live`}>
              <button type="button" role="radio" aria-checked={liveHereOn ? false : !withLive} disabled={optionsLocked} onClick={() => setLivePrefs({ on: false })}>{t("Off")}</button>
              <button type="button" role="radio" aria-checked={liveHereOn ? true : withLive} disabled={optionsLocked} title={liveUnavailable ?? undefined} onClick={() => setLivePrefs({ on: true })}>
                <span className="live-dot" aria-hidden="true" />{t("On")}
              </button>
            </div>
          </div>
          <div className={`room-setup-group ${withLive || liveHereOn ? "" : "is-muted"}`}>
            <span id={`${uidLabel}-public`}>{t("Quem pode assistir")}</span>
            <div className="room-chips" role="radiogroup" aria-labelledby={`${uidLabel}-public`}>
              <button type="button" role="radio" aria-checked={!(liveHereOn ? live.state.public : prefs.public)} disabled={optionsLocked || !withLive} onClick={() => setLivePrefs({ public: false })}
                title={t("Só quem tem o link da live assiste.")}>{t("Só com link")}</button>
              <button type="button" role="radio" aria-checked={liveHereOn ? live.state.public : prefs.public} disabled={optionsLocked || !withLive} onClick={() => setLivePrefs({ public: true })}
                title={t("Qualquer pessoa encontra a live no YouTube.")}>{t("Pública")}</button>
            </div>
          </div>
          <div className={`room-setup-group ${platform === "emulator" ? "" : "is-muted"}`}>
            <span id={`${uidLabel}-fullscreen`}>{t("Tela cheia na gravação")}</span>
            <div className="room-chips" role="radiogroup" aria-labelledby={`${uidLabel}-fullscreen`}>
              <button type="button" role="radio" aria-checked={!prefs.fullscreen} disabled={optionsLocked || platform !== "emulator"} title={fullscreenNote} onClick={() => setLivePrefs({ fullscreen: false })}>{t("Off")}</button>
              <button type="button" role="radio" aria-checked={prefs.fullscreen} disabled={optionsLocked || platform !== "emulator"} title={fullscreenNote} onClick={() => setLivePrefs({ fullscreen: true })}>{t("On")}</button>
            </div>
          </div>
          {/* "On" sem como transmitir (MiSTer ou sem canal): explica aqui, e o Play Ranked segue sem live. */}
          {liveUnavailable && !comingSoon && prefs.on && <p className="ranked-options-note" role="status">{liveUnavailable}</p>}
        </div>
      </div>
    </div>
  );
}
