import { useEffect, useId, useRef, useState, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import type { GameMode, GameRoom, PlayOn } from "../types/room";
import { liveActive, useLive, useYouTube } from "../data/useLive";
import { readEmulatorLibrary, romsFor } from "../data/emulatorLibrary";
import { isSupported } from "../data/validation";
import CreditsBadge from "./CreditsBadge";
import { useDialogFocus } from "./useDialogFocus";
import { useWallet } from "../data/useWallet";
import type { useGameLauncher } from "./useGameLauncher";

/** Opções do Play Ranked: transmitir no YouTube e se a live é pública. */
export interface LivePrefs { on: boolean; public: boolean }

// Lembradas entre as salas e a Coleção (as duas telas mostram e mudam as mesmas opções).
const KEY = "shmupx.client.livePrefs.v1";
const listeners = new Set<() => void>();
let cache: { raw: string | null; value: LivePrefs } | null = null;
function readLivePrefs(): LivePrefs {
  let raw: string | null = null;
  try { raw = localStorage.getItem(KEY); } catch { /* sem armazenamento: padrões */ }
  if (cache && cache.raw === raw) return cache.value;
  let value: LivePrefs = { on: false, public: false };
  try {
    const saved = JSON.parse(raw ?? "{}") as Partial<LivePrefs>;
    value = { on: saved.on === true, public: saved.public === true };
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
        await invoke("inicializar_jogo_emulador", { jogoId: room.id, roms: romsFor(room), pasta: readEmulatorLibrary()?.root || null, treino: true });
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

/** Depois de inserir a ficha: Training Mode (grátis, sem ranking) ou Ranked Mode (1 ficha). */
function PlayModeDialog({ game, withLive, rankedBlocked, onTraining, onRanked, onClose }: {
  game: string; withLive: boolean; rankedBlocked: string | null; onTraining: () => void; onRanked: () => void; onClose: () => void;
}) {
  const { t } = useI18n();
  const ref = useDialogFocus(true, onClose);
  return (
    <div className="controller-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby="play-mode-title" tabIndex={-1} className="controller-dialog play-mode-dialog">
        <header>
          <div>
            <h2 id="play-mode-title">{game}</h2>
            <p>{t("Como você quer jogar?")}</p>
          </div>
        </header>
        <div className="play-mode-choices">
          <button type="button" className="play-mode-choice" onClick={onTraining}>
            <strong>{t("Training Mode")}</strong>
            <span className="play-mode-cost is-free">{t("Grátis")}</span>
            <small>{t("Não gasta ficha. Não vale para o ranking, não grava a run e não transmite live.")}</small>
          </button>
          <button type="button" className="play-mode-choice is-ranked" onClick={onRanked} disabled={Boolean(rankedBlocked)} title={rankedBlocked ?? undefined}>
            <strong>{t("Ranked Mode")}{withLive && !rankedBlocked && <span className="play-live-chip">{t("Live")}</span>}</strong>
            <span className="play-mode-cost">{t("1 ficha")}</span>
            <small>{rankedBlocked ?? t(withLive ? "Gasta 1 ficha, grava a run, vale para o ranking e transmite a live no YouTube." : "Gasta 1 ficha, grava a run e vale para o ranking.")}</small>
          </button>
        </div>
        <footer><span /><button type="button" className="secondary-action" onClick={onClose}>{t("Cancelar")}</button></footer>
      </div>
    </div>
  );
}

/**
 * Play: a ficha (clique nela: ela gira e pergunta Training Mode ou Ranked Mode) com o saldo e, ao lado
 * ou abaixo, as opções da partida — live no YouTube e quem pode assistir.
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
  const liveLabel = live.state.status === "waiting" ? t("Abrindo o jogo…")
    : live.state.status === "preparing" ? t("Criando a live…")
    : live.state.status === "connecting" ? t("Conectando ao YouTube…")
    : live.state.status === "ending" ? t("Encerrando…")
    : t("End live");
  const optionsLocked = comingSoon || busy || liveOn;
  // Live deste jogo no ar (ou abrindo): as opções mostram como ela está.
  const liveHereOn = liveOn && liveHere;

  const credits = useWallet(uid);
  // Clique na ficha: ela gira e, quase no fim do giro, pergunta Training Mode ou Ranked Mode.
  const [choosing, setChoosing] = useState(false);
  const chooseTimer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(chooseTimer.current), []);
  function insertCoin() {
    setCoinSpin((n) => n + 1);
    window.clearTimeout(chooseTimer.current);
    const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    chooseTimer.current = window.setTimeout(() => setChoosing(true), reduced ? 0 : 1100);
  }
  /** Ranked Mode: gasta 1 ficha, grava e vale ranking; com Live On, abre o jogo já transmitindo. */
  function playRanked() {
    setChoosing(false);
    ranked.setLiveError(null);
    const kind = withLive ? "live" : "ranked";
    if (platform && (kind === "ranked" || platform !== "mister")) launcher.playNow(kind, platform, mode);
    else launcher.play(kind);
  }
  /** Training Mode: sem ficha, sem ranking, sem gravação e sem live. */
  function playTraining() {
    setChoosing(false);
    if (platform) launcher.playNow("training", platform, mode);
    else launcher.play("training");
  }
  // Ranked Mode indisponível: jogo sem script de validação ou sem fichas (o treino continua).
  const rankedBlocked = comingSoon
    ? t("Coming soon: Play Ranked fica disponível quando o jogo tiver o script de validação.")
    : credits !== null && credits < 1 ? t("Sem fichas: o Ranked Mode precisa de 1 ficha.") : null;

  return (
    <div className={`room-header-side ${compact ? "is-compact" : ""}`}>
      <CreditsBadge uid={uid} className="room-credits" spin={coinSpin} onPlay={insertCoin} live={withLive}
        playDisabled={launcher.isLaunching || busy || liveOn}
        playTitle={liveOn && !liveHere ? t("Há uma live em andamento em outra sala.") : t("Clique na ficha para jogar: Training Mode ou Ranked Mode.")} />
      {choosing && <PlayModeDialog game={room.title} withLive={withLive} rankedBlocked={rankedBlocked} onTraining={playTraining} onRanked={playRanked} onClose={() => setChoosing(false)} />}
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
          {/* "On" sem como transmitir (MiSTer ou sem canal): explica aqui, e o Play Ranked segue sem live. */}
          {liveUnavailable && !comingSoon && prefs.on && <p className="ranked-options-note" role="status">{liveUnavailable}</p>}
        </div>
      </div>
    </div>
  );
}
