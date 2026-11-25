import { useEffect, useId, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import type { GameMode, GameRoom, PlayOn } from "../types/room";
import { liveActive, useLive, useYouTube } from "../data/useLive";
import { readEmulatorLibrary, romsFor } from "../data/emulatorLibrary";
import { readEmulatorSettings } from "../data/emulatorSettings";
import { readEmulatorControls } from "../data/emulatorControls";
import { isSupported } from "../data/validation";
import CreditsBadge from "./CreditsBadge";
import { useDialogFocus } from "./useDialogFocus";
import { useWallet } from "../data/useWallet";
import { RANKED_MISTER_OFF, type useGameLauncher } from "./useGameLauncher";

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
    // "Quem pode assistir" saiu por enquanto: a live é sempre só com link (não listada).
    value = { on: saved.on === true, public: false };
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
        await invoke("inicializar_jogo_emulador", { jogoId: room.id, roms: romsFor(room), pasta: readEmulatorLibrary()?.root || null, treino: true, motorVideo: readEmulatorSettings(), motorControles: readEmulatorControls() });
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
const MODE_LABEL: Record<GameMode, string> = { highscore: "Hi-Score", "1cc": "1CC", caravan: "Caravan" };
const PLATFORM_LABEL: Record<PlayOn, string> = { steam: "Steam", emulator: "Emulador", mister: "MiSTer" };

function PlayModeDialog({ game, mode, platform, withLive, livePublic, rankedBlocked, onTraining, onRanked, onClose }: {
  game: string; mode: GameMode; platform: PlayOn | null; withLive: boolean; livePublic: boolean; rankedBlocked: string | null;
  onTraining: () => void; onRanked: () => void; onClose: () => void;
}) {
  const { t } = useI18n();
  // O que está marcado no painel: modo de jogo, plataforma e a live (com quem pode assistir).
  const rankedSetup = [
    { key: "mode", label: t(MODE_LABEL[mode]) },
    ...(platform ? [{ key: "platform", label: t(PLATFORM_LABEL[platform]) }] : []),
    ...(withLive ? [{ key: "live", label: `${t("Live")} · ${t(livePublic ? "Pública" : "Só com link")}`, live: true }] : []),
  ];
  const ref = useDialogFocus(true, onClose);
  // Atalhos: 1 = Training Mode, 2 = Ranked Mode (Esc fecha pelo useDialogFocus).
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.repeat || event.ctrlKey || event.metaKey || event.altKey) return;
      if (event.key === "1") { event.preventDefault(); onTraining(); }
      else if (event.key === "2" && !rankedBlocked) { event.preventDefault(); onRanked(); }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onTraining, onRanked, rankedBlocked]);
  return (
    <div className="controller-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby="play-mode-title" tabIndex={-1} className="controller-dialog play-mode-dialog">
        <header>
          <div>
            <h2 id="play-mode-title">{game}</h2>
            <p>{t("Como você quer jogar?")}</p>
          </div>
        </header>
        <div className="play-mode-choices" data-tutorial="play-mode-choices">
          <button type="button" className="play-mode-choice is-training" onClick={onTraining} aria-keyshortcuts="1">
            <span className="play-mode-art" aria-hidden="true" />
            <span className="play-mode-text">
              <strong>{t("Training Mode")}</strong>
              <small>{t("Não gasta ficha. Não vale para o ranking, não grava a run e não transmite live.")}</small>
            </span>
          </button>
          <button type="button" className="play-mode-choice is-ranked" onClick={onRanked} disabled={Boolean(rankedBlocked)} title={rankedBlocked ?? undefined} aria-keyshortcuts="2">
            <span className="play-mode-art" aria-hidden="true">
              <img className="play-mode-coin" src="/img/shmupx-token-front.png" alt="" draggable={false} />
            </span>
            <span className="play-mode-text">
              <strong>{t("Ranked Mode")}</strong>
              {rankedBlocked ? <small>{rankedBlocked}</small> : <>
                <span className="play-mode-setup">
                  {rankedSetup.map((item) => <span key={item.key} className={item.live ? "is-live" : undefined}>{item.label}</span>)}
                </span>
                <small>{t("Gasta 1 ficha, grava a run e vale para o ranking.")}</small>
              </>}
            </span>
          </button>
        </div>
        <footer><span className="play-mode-hint">{t("Aperte 1 ou 2 para escolher")}</span><button type="button" className="secondary-action" onClick={onClose}>{t("Cancelar")}</button></footer>
      </div>
    </div>
  );
}

/**
 * Play: a ficha (clique nela: ela gira e pergunta Training Mode ou Ranked Mode) com o saldo e as
 * opções da partida (modo de jogo e plataforma em `setup`, e a live no YouTube). Na sala, as opções
 * ficam empilhadas à esquerda e a ficha com o saldo alinhada à direita; no painel da Coleção
 * (`compact`), tudo numa coluna, com a ficha em cima.
 */
export default function RankedPanel({ room, uid, launcher, mode, platform, ranked, busy, compact = false, setup }: {
  room: GameRoom;
  uid: string | undefined;
  launcher: ReturnType<typeof useGameLauncher>;
  mode: GameMode;
  platform: PlayOn | null;
  ranked: ReturnType<typeof useRankedLive>;
  /** Modo de jogo e plataforma: na sala, em cima da live; na Coleção, logo abaixo da ficha. */
  setup?: ReactNode;
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
  // Plataforma da partida: a marcada no painel; sem nenhuma marcada (jogo ainda não importado),
  // a primeira em que ele roda — abre direto, e o aviso de jogo não encontrado explica se faltar.
  const target = platform ?? launcher.platforms[0] ?? null;
  const liveUnavailable = target === "mister"
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
    if (target && (kind === "ranked" || target !== "mister")) launcher.playNow(kind, target, mode);
    else launcher.play(kind);
  }
  /** Training Mode: sem ficha, sem ranking, sem gravação e sem live. */
  function playTraining() {
    setChoosing(false);
    if (target) launcher.playNow("training", target, mode);
    else launcher.play("training");
  }
  // Ranked Mode indisponível: jogo sem script de validação, no MiSTer sem leitura de placar, ou sem
  // fichas (o treino continua).
  const rankedBlocked = comingSoon
    ? t("Em breve: o Play Ranked fica disponível quando o jogo tiver o script de validação.")
    : target === "mister" && !room.rankedMister ? t(RANKED_MISTER_OFF)
    : credits !== null && credits < 1 ? t("Sem fichas: o Ranked Mode precisa de 1 ficha.") : null;

  const coin = (
    <CreditsBadge uid={uid} className="room-credits" spin={coinSpin} onPlay={insertCoin} live={withLive}
      playDisabled={launcher.isLaunching || busy || liveOn}
      playTitle={liveOn && !liveHere ? t("Há uma live em andamento em outra sala.") : t("Clique na ficha para jogar: Training Mode ou Ranked Mode.")} />
  );
  const dialog = choosing && <PlayModeDialog game={room.title} mode={mode} platform={target} withLive={withLive} livePublic={prefs.public} rankedBlocked={rankedBlocked} onTraining={playTraining} onRanked={playRanked} onClose={() => setChoosing(false)} />;
  const options = (
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
          {/* "On" sem como transmitir (MiSTer ou sem canal): explica aqui, e o Play Ranked segue sem live. */}
          {liveUnavailable && !comingSoon && prefs.on && <p className="ranked-options-note" role="status">{liveUnavailable}</p>}
        </div>
      </div>
  );

  if (compact) {
    return <div className="room-header-side is-compact">{coin}{setup}{dialog}{options}</div>;
  }
  return (
    <div className="room-header-side">
      <div className="room-side-stack">{setup}{options}</div>
      {coin}
      {dialog}
    </div>
  );
}
