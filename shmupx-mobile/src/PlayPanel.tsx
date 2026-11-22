import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "@client/i18n";
import { GAME_MODES, misterEntryMatches, modesFor } from "@client/data/gameModes";
import { romsFor } from "@client/data/emulatorLibrary";
import { misterMatches, useMisterLibrary } from "@client/data/misterLibrary";
import { misterAuth, readMisterPlayer, type MisterMenuGame } from "@client/data/misterPlayer";
import { desktopOnline, sendRemoteCommand, useRemoteLaunch, type RemotePlatform } from "@client/data/remoteLaunch";
import type { GameMode, GameRoom } from "@client/types/room";

type Kind = "training" | "ranked";
type Platform = RemotePlatform | "mister";

const LABEL: Record<Platform, string> = { steam: "Steam", emulator: "Emulador", mister: "MiSTer" };
const MARK: Record<Platform, string> = { steam: "S", emulator: "E", mister: "M" };
/** Sem resposta do PC nesse tempo: provavelmente o Client está fechado. */
const PC_TIMEOUT_MS = 25_000;

const slug = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
const describe = (error: unknown) => (error instanceof Error ? error.message : String(error));

/** O jogo roda no MiSTer? (Lab marcou MiSTer ou a biblioteca lida do MiSTer tem o jogo.) */
export function useMisterCompatible(room: GameRoom) {
  const library = useMisterLibrary();
  return room.platforms.includes("mister") || Boolean(library.library && misterMatches(room, library.library));
}

type Session =
  | { where: "mister"; host: string; password?: string; kind: Kind; label: string }
  | { where: "pc"; commandId: string; platform: RemotePlatform; kind: Kind; status: "waiting" | "started" | "failed" | "stopping"; message?: string; sentAt: number };

/**
 * Jogar pela sala no celular: Play Training (sem ficha) ou Play Ranked (gasta 1 ficha). Abre uma
 * folha com o modo de jogo e a plataforma: Steam e Emulador abrem no PC (o Client aberto na mesma
 * conta recebe o pedido), MiSTer abre na TV pela rede. PARAR JOGO encerra onde estiver.
 */
export default function PlayPanel({ room, uid, onSettings }: { room: GameRoom; uid: string; onSettings: () => void }) {
  const { t } = useI18n();
  const misterOk = useMisterCompatible(room);
  const remote = useRemoteLaunch(uid);
  const player = readMisterPlayer();
  const pcOnline = desktopOnline(remote?.desktop);
  const ownedOnPc = remote?.desktop?.owned?.[room.id] ?? [];
  const modes = GAME_MODES.filter((m) => modesFor(room).includes(m.id));

  const platforms: Platform[] = [];
  if (room.steamAppId > 0 || room.platforms.includes("steam")) platforms.push("steam");
  if (romsFor(room).length > 0 || room.platforms.includes("emulator")) platforms.push("emulator");
  if (misterOk) platforms.push("mister");

  const [sheet, setSheet] = useState<{ kind: Kind; mode: GameMode; platform: Platform | null } | null>(null);
  const [session, setSession] = useState<Session | null>(null);
  const [choices, setChoices] = useState<MisterMenuGame[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const usable = (p: Platform) => (p === "mister" ? true : pcOnline);
  const owned = (p: Platform) => (p === "mister" ? Boolean(player) : ownedOnPc.includes(p));

  function availability(p: Platform, kind: Kind): { text: string; ok: boolean } {
    if (p === "mister") {
      if (!player) return { text: t("Ligue o MiSTer em Ajustes para jogar na TV"), ok: false };
      return { text: kind === "training" ? t("Na TV, como CONVIDADO (sem ficha)") : t("Na TV, como {player}", { player: player.username.toUpperCase() }), ok: true };
    }
    if (!pcOnline) return { text: t("Abra o ShmupX no PC para jogar por lá"), ok: false };
    // Sem a biblioteca importada no Client, o PC confere na hora se o jogo está instalado.
    return { text: ownedOnPc.includes(p) ? t("No seu PC") : t("No seu PC (o Client confere se está instalado)"), ok: true };
  }

  function open(kind: Kind) {
    setError(null);
    setChoices(null);
    const preferred = platforms.find((p) => usable(p) && owned(p)) ?? platforms.find(usable) ?? platforms[0] ?? null;
    setSheet({ kind, mode: modes[0]?.id ?? "highscore", platform: preferred });
  }

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

  async function startMister(kind: Kind, mode: GameMode, game?: MisterMenuGame) {
    if (!player) { setSheet(null); onSettings(); return; }
    setBusy(true);
    setError(null);
    try {
      let target = game;
      if (!target) {
        const list = await invoke<MisterMenuGame[]>("mister_jogos_do_jogador", { host: player.host, ...misterAuth(player.password), username: kind === "training" ? "convidado" : player.username });
        const names = new Set([slug(room.title), slug(room.id)]);
        const ofGame = list.filter((g) => names.has(slug(g.jogo)));
        if (ofGame.length === 0) {
          setError(t("{game} não está no menu ShmupX desse jogador no MiSTer. Conecte o MiSTer de novo em Ajustes.", { game: room.title }));
          return;
        }
        const ofMode = ofGame.filter((g) => misterEntryMatches(g.modo, mode));
        if (ofMode.length === 1) target = ofMode[0];
        else if (ofGame.length === 1) target = ofGame[0];
        else { setChoices(ofMode.length > 1 ? ofMode : ofGame); return; }
      }
      await invoke("mister_iniciar", { host: player.host, ...misterAuth(player.password), caminho: target.caminho });
      setSheet(null);
      setChoices(null);
      setSession({ where: "mister", host: player.host, password: player.password, kind, label: target.modo });
    } catch (failure) {
      setError(describe(failure));
    } finally {
      setBusy(false);
    }
  }

  async function confirm() {
    if (!sheet?.platform) return;
    const { kind, mode, platform } = sheet;
    if (platform === "mister") { await startMister(kind, mode); return; }
    setBusy(true);
    setError(null);
    try {
      const commandId = await sendRemoteCommand(uid, { action: "play", gameId: room.id, platform, kind, mode });
      setSheet(null);
      setSession({ where: "pc", commandId, platform, kind, status: "waiting", sentAt: Date.now() });
    } catch (failure) {
      setError(describe(failure));
    } finally {
      setBusy(false);
    }
  }

  async function stop() {
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
        const commandId = await sendRemoteCommand(uid, { action: "stop", gameId: room.id });
        setSession({ ...session, commandId, status: "stopping" });
      }
    } catch (failure) {
      setError(describe(failure));
    } finally {
      setBusy(false);
    }
  }

  if (platforms.length === 0) return null;

  const sheetPlatform = sheet?.platform ?? null;
  const sheetAvailability = sheetPlatform && sheet ? availability(sheetPlatform, sheet.kind) : null;
  const canPlay = Boolean(sheetPlatform && sheetAvailability && (sheetAvailability.ok || (sheetPlatform === "mister" && !player)));

  return (
    <section className="lite-play" aria-label={t("Jogar")}>
      {session ? (
        <div className={`lite-session ${session.where === "pc" && session.status === "failed" ? "is-failed" : ""}`}>
          <p className="lite-session-status" role="status">
            <span className={`presence-dot ${session.where === "pc" && session.status === "failed" ? "is-offline" : "is-live"}`} aria-hidden="true" />
            <span>
              <strong>{session.where === "mister"
                ? t(session.kind === "training" ? "Treinando na TV" : "Jogando na TV")
                : t(session.status === "waiting" ? "Enviado ao PC…" : session.status === "stopping" ? "Encerrando no PC…" : session.status === "failed" ? "Não abriu no PC" : "Jogando no PC")}</strong>
              <small>{session.where === "mister"
                ? `${session.label} · ${t(session.kind === "training" ? "sem ficha" : "valendo ranking")}`
                : session.message ?? `${LABEL[session.platform]} · ${t(session.kind === "training" ? "sem ficha" : "valendo ranking")}`}</small>
            </span>
          </p>
          <button type="button" className={session.where === "pc" && session.status === "failed" ? "lite-secondary" : "lite-stop"} disabled={busy || (session.where === "pc" && session.status === "stopping")} onClick={() => void stop()}>
            {session.where === "pc" && session.status === "failed" ? t("Fechar") : t(busy ? "Encerrando o jogo…" : "PARAR JOGO")}
          </button>
        </div>
      ) : (
        <div className="lite-play-buttons">
          <button type="button" className="lite-play-training" onClick={() => open("training")}>
            <strong>{t("Play Training")}</strong><small>{t("sem ficha")}</small>
          </button>
          <button type="button" className="lite-play-ranked" onClick={() => open("ranked")}>
            <strong>{t("Play Ranked")}</strong><small>{t("gasta 1 ficha")}</small>
          </button>
        </div>
      )}
      {error && !sheet && <p className="lite-error" role="alert">{t(error)}</p>}

      {sheet && (
        <div className="lite-sheet-backdrop" onClick={(event) => { if (event.target === event.currentTarget && !busy) setSheet(null); }}>
          <div className="lite-sheet" role="dialog" aria-modal="true" aria-labelledby="lite-sheet-title">
            <div className="lite-sheet-grip" aria-hidden="true" />
            <header className="lite-sheet-header">
              <div>
                <h2 id="lite-sheet-title">{t(sheet.kind === "training" ? "Play Training" : "Play Ranked")}</h2>
                <p>{room.title} · {t(sheet.kind === "training" ? "sem ficha nem ranking" : "gasta 1 ficha e vale ranking")}</p>
              </div>
              <button type="button" className="lite-icon-button" aria-label={t("Fechar")} disabled={busy} onClick={() => setSheet(null)}>✕</button>
            </header>

            {choices ? (
              <div className="lite-sheet-body">
                <h3 className="lite-sheet-label">{t("Qual entrada do menu do MiSTer?")}</h3>
                <div className="lite-option-list">
                  {choices.map((game) => (
                    <button key={game.caminho} type="button" className="lite-option" disabled={busy} onClick={() => void startMister(sheet.kind, sheet.mode, game)}>
                      <span className="lite-option-mark" aria-hidden="true">M</span>
                      <span className="lite-option-text"><strong>{game.modo}</strong><small>{game.jogo}</small></span>
                    </button>
                  ))}
                </div>
              </div>
            ) : (
              <div className="lite-sheet-body">
                <h3 className="lite-sheet-label">{t("Modo de jogo")}</h3>
                <div className="lite-mode-chips" role="radiogroup" aria-label={t("Modo de jogo")}>
                  {modes.map((m) => (
                    <button key={m.id} type="button" role="radio" aria-checked={sheet.mode === m.id} className="lite-chip" onClick={() => setSheet({ ...sheet, mode: m.id })}>
                      <strong>{m.label}</strong><small>{t(m.detail)}</small>
                    </button>
                  ))}
                </div>

                <h3 className="lite-sheet-label">{t("Plataforma")}</h3>
                <div className="lite-option-list" role="radiogroup" aria-label={t("Plataforma")}>
                  {platforms.map((p) => {
                    const info = availability(p, sheet.kind);
                    return (
                      <button key={p} type="button" role="radio" aria-checked={sheet.platform === p} className={`lite-option ${info.ok ? "" : "is-unavailable"}`} onClick={() => setSheet({ ...sheet, platform: p })}>
                        <span className="lite-option-mark" aria-hidden="true">{MARK[p]}</span>
                        <span className="lite-option-text"><strong>{t(LABEL[p])}</strong><small>{info.text}</small></span>
                        <span className="lite-option-radio" aria-hidden="true" />
                      </button>
                    );
                  })}
                </div>
                {sheetPlatform && sheetPlatform !== "mister" && !pcOnline && (
                  <p className="lite-muted">{t("Steam e Emulador abrem no seu PC: deixe o ShmupX Client aberto no PC com a mesma conta e escolha aqui.")}</p>
                )}
              </div>
            )}

            {error && <p className="lite-error" role="alert">{t(error)}</p>}
            <footer className="lite-sheet-footer">
              {choices
                ? <button type="button" className="lite-secondary" disabled={busy} onClick={() => setChoices(null)}>{t("Voltar")}</button>
                : <button type="button" className="lite-primary" disabled={busy || !canPlay} onClick={() => void confirm()}>
                    {busy ? t("Abrindo…") : sheetPlatform === "mister" && !player ? t("Ligar o MiSTer em Ajustes") : sheetPlatform === "mister" ? t("Jogar na TV") : t("Jogar no PC")}
                  </button>}
            </footer>
          </div>
        </div>
      )}
    </section>
  );
}
