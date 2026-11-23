import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "@client/i18n";
import { GAME_MODES, misterEntryMatches, modesFor } from "@client/data/gameModes";
import { romsFor } from "@client/data/emulatorLibrary";
import { misterMatches, useMisterLibrary } from "@client/data/misterLibrary";
import { misterAuth, readMisterPlayer, type MisterMenuGame } from "@client/data/misterPlayer";
import { desktopOnline, sendRemoteCommand, useRemoteLaunch, type RemotePlatform } from "@client/data/remoteLaunch";
import { clearPlaying, playingActive, playingWhere, usePlayingClaim, type PlayingNow } from "@client/data/playing";
import type { GameMode, GameRoom } from "@client/types/room";
import type { Kind, Platform, PlayPreset } from "./prefs";
import { useBackLayer } from "./back";

export const PLATFORM_LABEL: Record<Platform, string> = { steam: "Steam", emulator: "Emulador", mister: "MiSTer" };
export const PLATFORM_MARK: Record<Platform, string> = { steam: "S", emulator: "E", mister: "M" };
export const ALL_PLATFORMS: Platform[] = ["steam", "emulator", "mister"];
/** Sem resposta do PC nesse tempo: provavelmente o Client está fechado. */
const PC_TIMEOUT_MS = 25_000;

const slug = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
const describe = (error: unknown) => (error instanceof Error ? error.message : String(error));

/** O jogo roda no MiSTer? (Lab marcou MiSTer ou a biblioteca lida do MiSTer tem o jogo.) */
export function useMisterCompatible(room: GameRoom) {
  const library = useMisterLibrary();
  return room.platforms.includes("mister") || Boolean(library.library && misterMatches(room, library.library));
}

/** Plataformas em que dá para jogar a sala pelo celular. */
export function platformsFor(room: GameRoom, misterOk: boolean): Platform[] {
  const list: Platform[] = [];
  if (room.steamAppId > 0 || room.platforms.includes("steam")) list.push("steam");
  if (romsFor(room).length > 0 || room.platforms.includes("emulator")) list.push("emulator");
  if (misterOk) list.push("mister");
  return list;
}

/** Modos da sala, com o da pré-configuração quando a sala tem ele. */
export function modeFor(room: GameRoom, wanted: GameMode): GameMode {
  const modes = modesFor(room);
  return modes.includes(wanted) ? wanted : modes[0] ?? "highscore";
}
export const roomModes = (room: GameRoom) => GAME_MODES.filter((m) => modesFor(room).includes(m.id));

export type Session =
  | { where: "mister"; room: GameRoom; host: string; password?: string; kind: Kind; label: string }
  | { where: "pc"; room: GameRoom; commandId: string; platform: RemotePlatform; kind: Kind; status: "waiting" | "started" | "failed" | "stopping"; message?: string };

type Choices = { room: GameRoom; kind: Kind; mode: GameMode; list: MisterMenuGame[] };

interface PlayApi {
  session: Session | null;
  /** Jogo aberto na conta por outro caminho (o PC, ou o MiSTer de uma sessão anterior do app). */
  external: PlayingNow | null;
  stopExternal: () => Promise<void>;
  busy: boolean;
  /** Último erro ao jogar e a sala dele (`null` = erro geral). */
  error: { roomId: string | null; message: string } | null;
  clearError: () => void;
  pcOnline: boolean;
  /** Plataformas em que o PC já sabe que o jogo está instalado. */
  ownedOnPc: (roomId: string) => string[];
  availability: (room: GameRoom, platform: Platform, kind: Kind) => { text: string; ok: boolean };
  /** Abre o jogo; `true` quando abriu (ou quando falta escolher a entrada do menu do MiSTer). */
  start: (room: GameRoom, kind: Kind, mode: GameMode, platform: Platform) => Promise<boolean>;
  /** ▶ da lista: joga com a pré-configuração, sem passar pela sala. */
  quickPlay: (room: GameRoom, preset: PlayPreset, misterOk: boolean) => Promise<boolean>;
  stop: () => Promise<void>;
}

const PlayContext = createContext<PlayApi | null>(null);

export function usePlay(): PlayApi {
  const api = useContext(PlayContext);
  if (!api) throw new Error("usePlay fora do PlayProvider");
  return api;
}

/**
 * Jogar pelo celular (lista ou sala): Play Training (sem ficha) ou Play Ranked (gasta 1 ficha).
 * Steam e Emulador abrem no PC (o Client aberto na mesma conta recebe o pedido), MiSTer abre na TV
 * pela rede. Um jogo por vez; PARAR JOGO encerra onde estiver.
 */
export function PlayProvider({ uid, onSettings, children }: { uid: string; onSettings: () => void; children: ReactNode }) {
  const { t } = useI18n();
  const remote = useRemoteLaunch(uid);
  const pcOnline = desktopOnline(remote?.desktop);
  const [session, setSession] = useState<Session | null>(null);
  const [choices, setChoices] = useState<Choices | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<PlayApi["error"]>(null);

  // Um jogo por vez na conta: o jogo aberto no MiSTer por aqui fica declarado na conta.
  usePlayingClaim(uid, session?.where === "mister"
    ? { gameId: session.room.id, title: session.room.title, platform: "mister", kind: session.kind, device: "lite" }
    : null);
  const accountPlaying = playingActive(remote?.playing) ? remote!.playing! : null;
  // Jogo aberto que este app não está acompanhando (aberto pelo PC, ou num uso anterior do app).
  const external = session ? null : accountPlaying;

  const ownedOnPc = useCallback((roomId: string) => remote?.desktop?.owned?.[roomId] ?? [], [remote?.desktop]);

  const availability = useCallback((room: GameRoom, p: Platform, kind: Kind) => {
    if (p === "mister") {
      const player = readMisterPlayer();
      if (!player) return { text: t("Ligue o MiSTer em Ajustes para jogar na TV"), ok: false };
      return { text: kind === "training" ? t("Na TV, como CONVIDADO (sem ficha)") : t("Na TV, como {player}", { player: player.username.toUpperCase() }), ok: true };
    }
    if (!pcOnline) return { text: t("Abra o ShmupX no PC para jogar por lá"), ok: false };
    // Sem a biblioteca importada no Client, o PC confere na hora se o jogo está instalado.
    return { text: ownedOnPc(room.id).includes(p) ? t("No seu PC") : t("No seu PC (o Client confere se está instalado)"), ok: true };
  }, [pcOnline, ownedOnPc, t]);

  // Resposta do PC ao pedido.
  useEffect(() => {
    if (session?.where !== "pc") return;
    const result = remote?.result;
    if (!result || result.id !== session.commandId) return;
    if (result.status === "stopped") { setSession(null); return; }
    if (result.status !== session.status) setSession({ ...session, status: result.status, message: result.message });
  }, [remote?.result, session]);

  // PC sem resposta.
  useEffect(() => {
    if (session?.where !== "pc" || session.status !== "waiting") return;
    const timer = window.setTimeout(() => {
      setSession((current) => current?.where === "pc" && current.status === "waiting"
        ? { ...current, status: "failed", message: t("O PC não respondeu. Confira se o ShmupX está aberto no PC, com a mesma conta.") }
        : current);
    }, PC_TIMEOUT_MS);
    return () => window.clearTimeout(timer);
  }, [session, t]);

  const launchMister = useCallback(async (room: GameRoom, kind: Kind, mode: GameMode, game?: MisterMenuGame): Promise<boolean> => {
    const player = readMisterPlayer();
    if (!player) { onSettings(); return false; }
    setBusy(true);
    setError(null);
    try {
      let target = game;
      if (!target) {
        const list = await invoke<MisterMenuGame[]>("mister_jogos_do_jogador", { host: player.host, ...misterAuth(player.password), username: kind === "training" ? "convidado" : player.username });
        const names = new Set([slug(room.title), slug(room.id)]);
        const ofGame = list.filter((g) => names.has(slug(g.jogo)));
        if (ofGame.length === 0) {
          setError({ roomId: room.id, message: t("{game} não está no menu ShmupX desse jogador no MiSTer. Conecte o MiSTer de novo em Ajustes.", { game: room.title }) });
          return false;
        }
        const ofMode = ofGame.filter((g) => misterEntryMatches(g.modo, mode));
        if (ofMode.length === 1) target = ofMode[0];
        else if (ofGame.length === 1) target = ofGame[0];
        else { setChoices({ room, kind, mode, list: ofMode.length > 1 ? ofMode : ofGame }); return true; }
      }
      await invoke("mister_iniciar", { host: player.host, ...misterAuth(player.password), caminho: target.caminho });
      setChoices(null);
      setSession({ where: "mister", room, host: player.host, password: player.password, kind, label: target.modo });
      return true;
    } catch (failure) {
      setError({ roomId: room.id, message: describe(failure) });
      return false;
    } finally {
      setBusy(false);
    }
  }, [onSettings, t]);

  const start = useCallback(async (room: GameRoom, kind: Kind, mode: GameMode, platform: Platform): Promise<boolean> => {
    if (session) { setError({ roomId: room.id, message: t("Já tem um jogo aberto: pare o {game} antes de abrir outro.", { game: session.room.title }) }); return false; }
    if (accountPlaying) {
      const where = playingWhere(accountPlaying);
      setError({ roomId: room.id, message: t("Já tem um jogo aberto na sua conta: {game} ({platform}, {device}). Pare esse jogo antes de abrir outro.", { game: accountPlaying.title, platform: t(where.platform), device: t(where.device) }) });
      return false;
    }
    if (platform === "mister") return launchMister(room, kind, mode);
    if (!pcOnline) { setError({ roomId: room.id, message: t("Abra o ShmupX no PC para jogar por lá") }); return false; }
    setBusy(true);
    setError(null);
    try {
      const commandId = await sendRemoteCommand(uid, { action: "play", gameId: room.id, platform, kind, mode });
      setSession({ where: "pc", room, commandId, platform, kind, status: "waiting" });
      return true;
    } catch (failure) {
      setError({ roomId: room.id, message: describe(failure) });
      return false;
    } finally {
      setBusy(false);
    }
  }, [session, accountPlaying, launchMister, pcOnline, uid, t]);

  const quickPlay = useCallback(async (room: GameRoom, preset: PlayPreset, misterOk: boolean) => {
    if (!platformsFor(room, misterOk).includes(preset.platform)) {
      setError({ roomId: room.id, message: t("{game} não abre em {platform}. Escolha outra plataforma acima ou entre na sala.", { game: room.title, platform: t(PLATFORM_LABEL[preset.platform]) }) });
      return false;
    }
    return start(room, preset.kind, modeFor(room, preset.mode), preset.platform);
  }, [start, t]);

  const stop = useCallback(async () => {
    if (!session) return;
    setBusy(true);
    setError(null);
    try {
      if (session.where === "mister") {
        await invoke("mister_parar", { host: session.host, ...misterAuth(session.password) });
        setSession(null);
      } else if (session.status === "failed") {
        setSession(null);
      } else {
        const commandId = await sendRemoteCommand(uid, { action: "stop", gameId: session.room.id });
        setSession({ ...session, commandId, status: "stopping" });
      }
    } catch (failure) {
      setError({ roomId: session.room.id, message: describe(failure) });
    } finally {
      setBusy(false);
    }
  }, [session, uid]);

  /** PARAR JOGO de um jogo aberto por outro caminho: no MiSTer volta ao menu; no PC pede ao Client. */
  const stopExternal = useCallback(async () => {
    if (!external) return;
    setBusy(true);
    setError(null);
    try {
      if (external.platform === "mister") {
        const player = readMisterPlayer();
        // Mesmo que o MiSTer já esteja no menu (ou fora da rede), a conta é liberada em seguida.
        if (player) await invoke("mister_parar", { host: player.host, ...misterAuth(player.password) }).catch(() => undefined);
        await clearPlaying(uid);
      } else {
        await sendRemoteCommand(uid, { action: "stop", gameId: external.gameId });
      }
    } catch (failure) {
      setError({ roomId: null, message: describe(failure) });
    } finally {
      setBusy(false);
    }
  }, [external, uid]);

  const api: PlayApi = { session, external, stopExternal, busy, error, clearError: () => setError(null), pcOnline, ownedOnPc, availability, start, quickPlay, stop };
  return (
    <PlayContext.Provider value={api}>
      {children}
      {choices && <MisterChoices choices={choices} busy={busy} onPick={(game) => void launchMister(choices.room, choices.kind, choices.mode, game)} onClose={() => setChoices(null)} />}
    </PlayContext.Provider>
  );
}

/** O jogador tem mais de uma entrada desse jogo no menu do MiSTer: escolhe qual abrir. */
function MisterChoices({ choices, busy, onPick, onClose }: { choices: Choices; busy: boolean; onPick: (game: MisterMenuGame) => void; onClose: () => void }) {
  const { t } = useI18n();
  useBackLayer(true, onClose);
  return (
    <div className="lite-sheet-backdrop" onClick={(event) => { if (event.target === event.currentTarget && !busy) onClose(); }}>
      <div className="lite-sheet" role="dialog" aria-modal="true" aria-labelledby="lite-choices-title">
        <div className="lite-sheet-grip" aria-hidden="true" />
        <header className="lite-sheet-header">
          <div>
            <h2 id="lite-choices-title">{choices.room.title}</h2>
            <p>{t("Qual entrada do menu do MiSTer?")}</p>
          </div>
          <button type="button" className="lite-icon-button" aria-label={t("Fechar")} disabled={busy} onClick={onClose}>✕</button>
        </header>
        <div className="lite-sheet-body">
          <div className="lite-option-list">
            {choices.list.map((game) => (
              <button key={game.caminho} type="button" className="lite-option" disabled={busy} onClick={() => onPick(game)}>
                <span className="lite-option-mark" aria-hidden="true">M</span>
                <span className="lite-option-text"><strong>{game.modo}</strong><small>{game.jogo}</small></span>
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

/** Jogo aberto agora (em qualquer sala) com o PARAR JOGO. */
const PLAYING_TITLE = { training: "Jogando em modo treino", ranked: "Jogando em modo ranqueado", live: "Jogando ao vivo" } as const;

export function SessionBar() {
  const { t } = useI18n();
  const { session, external, stopExternal, busy, stop } = usePlay();
  if (!session && external) {
    const where = playingWhere(external);
    return (
      <div className="lite-session">
        <p className="lite-session-status" role="status">
          <span className="presence-dot is-live" aria-hidden="true" />
          <span><strong>{t(PLAYING_TITLE[external.kind])}</strong><small>{external.title} · {t(where.platform)} · {t(where.device)}</small></span>
        </p>
        <button type="button" className="lite-stop" disabled={busy} onClick={() => void stopExternal()}>{t(busy ? "Encerrando o jogo…" : "PARAR JOGO")}</button>
      </div>
    );
  }
  if (!session) return null;
  const failed = session.where === "pc" && session.status === "failed";
  const title = session.where === "mister" || session.status === "started"
    ? t(PLAYING_TITLE[session.kind])
    : t(session.status === "waiting" ? "Enviado ao PC…" : session.status === "stopping" ? "Encerrando no PC…" : "Não abriu no PC");
  const detail = session.where === "mister"
    ? `${session.room.title} · MiSTer · ${session.label}`
    : session.message ?? `${session.room.title} · ${t(PLATFORM_LABEL[session.platform])} · ${t("no PC")}`;
  return (
    <div className={`lite-session ${failed ? "is-failed" : ""}`}>
      <p className="lite-session-status" role="status">
        <span className={`presence-dot ${failed ? "is-offline" : "is-live"}`} aria-hidden="true" />
        <span><strong>{title}</strong><small>{detail}</small></span>
      </p>
      <button type="button" className={failed ? "lite-secondary" : "lite-stop"} disabled={busy || (session.where === "pc" && session.status === "stopping")} onClick={() => void stop()}>
        {failed ? t("Fechar") : t(busy ? "Encerrando o jogo…" : "PARAR JOGO")}
      </button>
    </div>
  );
}
