import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../i18n";
import type { GameMode, GameRoom, PlayOn } from "../types/room";
import { rankedLaunchFields } from "../data/playerCard";
import { caravanSecondsOf, misterEntryMatches, playableMode, playModesFor, setPendingRunEvent, setPendingRunMode } from "../data/gameModes";
import { readEmulatorLibrary, romsFor, systemFor } from "../data/emulatorLibrary";
import { emulatorVideoForLaunch } from "../data/emulatorSettings";
import { readEmulatorControls } from "../data/emulatorControls";
import { misterAuth, readMisterPlayer, saveMisterPlayer, type MisterMenuGame } from "../data/misterPlayer";
import { readMisterLibrary } from "../data/misterLibrary";
import { PLAY_ON, installCheck, usePlatformOwnership } from "../data/platforms";
import { checkMister, logFailure } from "../data/launchFailure";
import { LaunchProblemDialog, type LaunchProblemKind } from "./LaunchProblemDialog";
import { useDialogFocus } from "./useDialogFocus";
import { blocksLaunch, getAccountPlaying, playingWhere, RANKED_MISTER_OFF, setClientMisterGame, useClientMisterGame } from "../data/playing";

/** Plataformas em que o jogo roda fora do PC: o Client não tem como abri-lo. */
const OWN_HARDWARE: Record<string, string> = { mister: "MiSTer", xbox360: "Xbox 360" };
export { RANKED_MISTER_OFF } from "../data/playing";

/** Nome comparável (sem acento, minúsculas, hífens): o menu do MiSTer usa o título do jogo. */
const slug = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

/**
 * Abrir o jogo (INICIAR JOGO e MODO TREINO), usado pela sala e pelo painel da coleção: escolhe a
 * plataforma (Steam, emulador ou MiSTer, conforme as bibliotecas do jogador), confere se o jogo
 * continua instalado e abre. No treino, o jogo abre sem gravar run nem gastar ficha (o Rust avisa
 * a abertura com `treino`, e o App não inicia a corrida); o MiSTer fica de fora, porque o agente
 * dele registra toda partida aberta pelo menu do jogador.
 */
/** Play Training (sem ficha nem gravação), Play Ranked (gasta 1 ficha e grava) ou Play Live Ranked. */
export type PlayKind = "training" | "ranked" | "live";

export function useGameLauncher(room: GameRoom, { onLive, recordingWindowed = false }: {
  onLive?: (platform: PlayOn, mode: GameMode) => void;
  /** Partidas valendo ranking (gravadas) no emulador abrem em janela, e não em tela cheia. */
  recordingWindowed?: boolean;
} = {}) {
  const { t } = useI18n();
  const playable = usePlatformOwnership();
  const [isLaunching, setIsLaunching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [practice, setPractice] = useState(false);
  /** Jogo aberto no MiSTer por aqui (o PARAR JOGO volta o MiSTer ao menu). */
  const [misterGame, setMisterGame] = useState<{ host: string; password?: string; practice: boolean } | null>(null);
  const [isStopping, setIsStopping] = useState(false);
  // O jogo do MiSTer saiu da conta por fora deste PARAR JOGO (STOP do topo, ou o agente avisou que
  // o MiSTer voltou ao menu): a sala volta ao estado de antes, como no PARAR JOGO.
  const clientMister = useClientMisterGame();
  useEffect(() => {
    if (!misterGame || clientMister?.gameId === room.id) return;
    setNotice(misterGame.practice
      ? t("Treino encerrado: o MiSTer voltou ao menu.")
      : t("O MiSTer voltou ao menu: o agente registrou a partida com o placar."));
    setMisterGame(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [clientMister]);
  /** Janela do Play: modo de jogo e plataforma, escolhidos antes de abrir. */
  const [choosing, setChoosing] = useState<{ kind: PlayKind; platforms: PlayOn[]; mode: GameMode; platform: PlayOn } | null>(null);
  const chooseDialogRef = useDialogFocus(choosing !== null, () => setChoosing(null));
  /** Modo escolhido no Play (o MiSTer abre a entrada do menu desse modo). */
  const [mode, setMode] = useState<GameMode | null>(null);
  /** Modos do jogo no menu ShmupX do MiSTer, para escolher qual abrir na TV. */
  const [misterModes, setMisterModes] = useState<MisterMenuGame[] | null>(null);
  const misterDialogRef = useDialogFocus(misterModes !== null, () => setMisterModes(null));
  /** Ligar o MiSTer na hora (IP e jogador), quando o jogador escolhe MiSTer sem ter conectado. */
  const [linking, setLinking] = useState<{ host: string; players: string[] | null; busy: boolean; error: string | null } | null>(null);
  const linkDialogRef = useDialogFocus(linking !== null, () => setLinking(null));
  /** O jogo não abriu (MiSTer fora da rede, jogo desinstalado, ROM faltando…): aviso simples. */
  const [problem, setProblem] = useState<{ kind: LaunchProblemKind; retry?: () => void } | null>(null);

  useEffect(() => {
    setError(null);
    setNotice(null);
    setChoosing(null);
    setMisterModes(null);
    setProblem(null);
    setMode(null);
    setIsLaunching(false);
    setMisterGame(null);
  }, [room.id]);

  /**
   * Por onde dá para abrir: todas as plataformas em que o jogo roda (as do Lab e as em que o
   * jogador já tem o jogo) e o que o Client sabe abrir (Steam pelo app_id, emulador pelo ROM).
   * Com mais de uma, o jogador escolhe; a janela mostra em quais ele já tem o jogo.
   */
  function options(): PlayOn[] {
    const ids = new Set<PlayOn>(playable.tags(room).map((tag) => tag.id));
    if (room.steamAppId > 0) ids.add("steam");
    if (romsFor(room).length > 0) ids.add("emulator");
    return PLAY_ON.map((p) => p.id).filter((id) => ids.has(id));
  }

  /**
   * Um jogo valendo ranking por vez na conta: com um Ranked/Live aberto no celular, no MiSTer ou na
   * Steam, avisa e não abre outro Ranked/Live. O do ShmupX Emulator deste PC pode ser interrompido
   * por outro no emulador (do mesmo jogo ou de outro): o jogo troca na mesma janela e a run anterior
   * termina na troca. Training não trava nem é travado. `platform`: onde o novo vai abrir (sem ela,
   * ainda na escolha, só trava o que não dá para interromper daqui).
   */
  function blockedByOpenGame(kind: PlayKind, platform?: PlayOn): boolean {
    const open = getAccountPlaying();
    if (!blocksLaunch(open, kind)) return false;
    if (open.device === "pc" && open.platform === "emulator" && (platform === undefined || platform === "emulator")) return false;
    const where = playingWhere(open);
    setError(t("Já tem um jogo aberto na sua conta: {game} ({platform}, {device}). Pare esse jogo antes de abrir outro.", { game: open.title, platform: t(where.platform), device: t(where.device) }));
    return true;
  }

  /** Play Training / Play Ranked / Play Live Ranked: abre a janela de modo e plataforma. */
  function play(kind: PlayKind) {
    setError(null);
    setNotice(null);
    if (blockedByOpenGame(kind)) return;
    setPractice(kind === "training");
    // A live captura a janela do jogo neste PC: o MiSTer (na TV) fica de fora. Ranked no MiSTer só
    // nos jogos em que o ShmupX lê o placar da memória lá (flag do Lab).
    const all = options().filter((id) => kind !== "live" || id !== "mister");
    const list = all.filter((id) => kind === "training" || id !== "mister" || room.rankedMister);
    if (list.length === 0 && all.includes("mister")) { setError(t(RANKED_MISTER_OFF)); return; }
    if (list.length === 0) {
      setError(room.platform && OWN_HARDWARE[room.platform]
          ? t("Este jogo roda no {platform}: abra-o no próprio aparelho.", { platform: OWN_HARDWARE[room.platform] })
          : t("O Client ainda não sabe abrir este jogo. Importe o jogo de uma plataforma em Importar jogos."));
      return;
    }
    const owned = list.find((id) => playable.owns(room, id));
    setChoosing({ kind, platforms: list, mode: playableMode(room, undefined), platform: owned ?? list[0] });
  }

  /**
   * Abre direto, com o modo e a plataforma já escolhidos no cabeçalho da sala (sem a janela).
   * Live: a sala põe a live no ar (o MiSTer fica de fora, porque a live captura a janela no PC).
   * `eventId`: partida valendo no evento (Event Mode); sem ele, a run não vale em nenhum evento.
   */
  function playNow(kind: PlayKind, platform: PlayOn, wanted: GameMode, eventId?: string) {
    // Só Hi-Score (e Caravan quando sair): o 1CC sai da run de Hi-Score.
    const chosen = playableMode(room, wanted);
    setError(null);
    setNotice(null);
    if (blockedByOpenGame(kind, platform)) return;
    if (kind !== "training" && platform === "mister" && !room.rankedMister) { setError(t(RANKED_MISTER_OFF)); return; }
    setPractice(kind === "training");
    setMode(chosen);
    // Live: a run ranked começa quando a live abrir o jogo; leva o modo escolhido também.
    if (kind !== "training" && platform !== "mister") setPendingRunMode(room.id, chosen);
    setPendingRunEvent(room.id, kind !== "training" && platform !== "mister" ? eventId ?? null : null);
    if (kind === "live") { onLive?.(platform, chosen); return; }
    void launchOn(platform, kind === "training", chosen);
  }

  /** Compatível com as telas antigas: `start(true)` é o Play Training, `start(false)` o Play Ranked. */
  function start(forPractice: boolean) { play(forPractice ? "training" : "ranked"); }

  function confirmPlay() {
    if (!choosing) return;
    const { kind, platform, mode: chosen } = choosing;
    setChoosing(null);
    if (blockedByOpenGame(kind, platform)) return;
    setMode(chosen);
    // A run começa quando o jogo abrir (PC; com live, depois que ela entra no ar): leva o modo.
    if (kind !== "training" && platform !== "mister") setPendingRunMode(room.id, chosen);
    setPendingRunEvent(room.id, null);
    if (kind === "live") { onLive?.(platform, chosen); return; }
    void launchOn(platform, kind === "training", chosen);
  }

  /** Abre o jogo no MiSTer pela rede (SSH): no menu ShmupX, com o jogador da conta e o modo. */
  async function openOnMister(game?: MisterMenuGame, forPractice = practice, chosenMode = mode) {
    const player = readMisterPlayer();
    if (!player) {
      // Ainda não sabe qual jogador do MiSTer é este: pergunta aqui mesmo (IP e jogador).
      const host = readMisterLibrary()?.host ?? "";
      setLinking({ host, players: null, busy: false, error: null });
      if (host) void loadMisterPlayers(host);
      return;
    }
    setIsLaunching(true);
    const retry = () => void openOnMister(game, forPractice, chosenMode);
    try {
      const reach = await checkMister(player.host);
      if (reach) { setProblem({ kind: `mister-${reach}`, retry }); return; }
      let target = game;
      if (!target) {
        // Treino no MiSTer: pelo CONVIDADO (sem token), sem ficha e sem ranking online.
        const username = forPractice ? "convidado" : player.username;
        const games = await invoke<MisterMenuGame[]>("mister_jogos_do_jogador", { host: player.host, ...misterAuth(player.password), username });
        const names = new Set([slug(room.title), slug(room.id)]);
        const modes = games.filter((g) => names.has(slug(g.jogo)));
        if (modes.length === 0) {
          logFailure("mister", `${room.title} não está no menu ShmupX de ${username} em ${player.host}`);
          setProblem({ kind: "mister-not-in-menu" });
          return;
        }
        // Entrada do menu do modo escolhido no Play; sem uma só que bata, o jogador escolhe.
        const ofMode = chosenMode ? modes.filter((g) => misterEntryMatches(g.modo, chosenMode)) : [];
        if (ofMode.length === 1) target = ofMode[0];
        else if (modes.length > 1) { setMisterModes(ofMode.length > 1 ? ofMode : modes); return; }
        else target = modes[0];
      }
      setMisterModes(null);
      await invoke("mister_iniciar", { host: player.host, ...misterAuth(player.password), caminho: target.caminho });
      setMisterGame({ host: player.host, password: player.password, practice: forPractice });
      setClientMisterGame({ gameId: room.id, title: room.title, kind: forPractice ? "training" : "ranked" });
      setNotice(forPractice
        ? t("Modo treino: abrindo {game} no MiSTer ({mode}) como CONVIDADO, sem gastar ficha nem ir para o ranking online.", { game: room.title, mode: target.modo })
        : t("Abrindo {game} no MiSTer ({mode}) como {player}. Jogue na TV: a partida entra no seu perfil.", { game: room.title, mode: target.modo, player: player.username.toUpperCase() }));
    } catch (failure) {
      logFailure("mister", failure);
      setProblem({ kind: "mister-failed", retry });
    } finally {
      setIsLaunching(false);
    }
  }

  async function loadMisterPlayers(host: string) {
    setLinking({ host, players: null, busy: true, error: null });
    const reach = await checkMister(host.trim());
    if (reach) {
      setLinking({ host, players: null, busy: false, error: reach === "outra_rede"
        ? t("Este computador e o MiSTer estão em redes diferentes. Conecte os dois à mesma internet de casa.")
        : reach === "sem_rede"
          ? t("Este computador está sem internet. Conecte-o à internet de casa e tente de novo.")
          : t("O MiSTer não respondeu. Confira o endereço e se ele está ligado e conectado à internet de casa.") });
      return;
    }
    try {
      const players = await invoke<string[]>("mister_usuarios", { host: host.trim(), ...misterAuth() });
      setLinking({ host, players, busy: false, error: players.length ? null : t("Nenhum jogador no shmupx.ini desse MiSTer. Use Conectar ao meu MiSTer em Importar jogos para instalar o ShmupX nele.") });
    } catch (failure) {
      logFailure("mister", failure);
      setLinking({ host, players: null, busy: false, error: t("Não foi possível ler o MiSTer. Confira o endereço e tente de novo.") });
    }
  }

  function chooseMisterPlayer(username: string) {
    if (!linking) return;
    saveMisterPlayer({ host: linking.host.trim(), username });
    setLinking(null);
    void openOnMister();
  }

  /** PARAR JOGO no MiSTer: volta ao menu; o agente grava a partida com o placar. */
  async function stopMister() {
    if (!misterGame) return;
    setIsStopping(true);
    setError(null);
    try {
      await invoke("mister_parar", { host: misterGame.host, ...misterAuth(misterGame.password) });
      setNotice(misterGame.practice
        ? t("Treino encerrado: o MiSTer voltou ao menu.")
        : t("O MiSTer voltou ao menu: o agente registrou a partida com o placar."));
      setMisterGame(null);
      setClientMisterGame(null);
    } catch (failure) {
      logFailure("mister", failure);
      const reach = await checkMister(misterGame.host);
      setProblem({ kind: reach ? `mister-${reach}` : "mister-stop-failed", retry: () => void stopMister() });
    } finally {
      setIsStopping(false);
    }
  }

  /**
   * Confere se o jogo está instalado (Steam) ou com o ROM na pasta de ROMs (ShmupX Emulator) antes de abrir;
   * se não, mostra o aviso do que falta. Usado também pela live da sala.
   */
  async function ensureInstalled(platform: PlayOn, retry?: () => void): Promise<boolean> {
    if (platform === "mister") return true;
    const check = await installCheck(room, platform);
    if (check === "ok") return true;
    logFailure(platform === "steam" ? "steam" : "emulador", `${room.title}: ${check === "no-app" ? (platform === "steam" ? "Steam não encontrada" : "ShmupX Emulator não encontrado") : "jogo/ROM não encontrado"}`);
    setProblem({ kind: platform === "steam" ? (check === "no-app" ? "steam-app-missing" : "steam-missing") : (check === "no-app" ? "emulator-missing" : "rom-missing"), retry });
    return false;
  }

  /**
   * A Steam às vezes não abre o jogo (atualização pendente, termos para aceitar, nuvem sincronizando
   * ou uma janela dela escondida): sem o jogo aparecer no tempo limite, avisa com o que fazer.
   */
  async function watchSteamLaunch(appId: number, retry: () => void) {
    const stops: (() => void)[] = [];
    const stop = () => stops.splice(0).forEach((unlisten) => unlisten());
    stops.push(await listen<{ app_id: number }>("steam://game-launched", (event) => { if (event.payload.app_id === appId) stop(); }));
    stops.push(await listen<{ app_id: number }>("steam://game-launch-timeout", (event) => {
      if (event.payload.app_id !== appId) return;
      stop();
      logFailure("steam", `${room.title} (app ${appId}): a Steam não abriu o jogo no tempo limite`);
      setProblem({ kind: "steam-timeout", retry });
    }));
  }

  async function launchOn(platform: PlayOn, forPractice = practice, chosenMode: GameMode | null = mode) {
    setChoosing(null);
    setError(null);
    setNotice(null);
    if (platform === "mister") {
      await openOnMister(undefined, forPractice, chosenMode);
      return;
    }
    setIsLaunching(true);
    const retry = () => void launchOn(platform, forPractice, chosenMode);
    try {
      if (!(await ensureInstalled(platform, retry))) return;
      if (platform === "steam") {
        if (room.steamAppId === 0) throw "Este jogo não tem app_id da Steam no catálogo.";
        await watchSteamLaunch(room.steamAppId, retry);
        await invoke("inicializar_jogo_steam", { steamAppId: room.steamAppId, treino: forPractice, semSteam: room.noSteamProtection === true });
      } else {
        await invoke("inicializar_jogo_emulador", { jogoId: room.id, roms: romsFor(room), sistema: systemFor(room), pasta: readEmulatorLibrary()?.root || null, treino: forPractice, janela: !forPractice && recordingWindowed, motorVideo: emulatorVideoForLaunch(), motorControles: readEmulatorControls(),
          ...(await rankedLaunchFields(caravanSecondsOf(room), chosenMode === "caravan", !forPractice)) });
      }
      if (forPractice) setNotice(t("Modo treino: {game} abre sem gastar ficha.", { game: room.title }));
    } catch (failure) {
      logFailure(platform === "steam" ? "steam" : "emulador", failure);
      // Ranked sem o estado de 1 crédito no pacote do motor (motor antigo): pede a atualização.
      const semEstado = platform !== "steam" && String(failure).includes("ainda não tem o Ranked");
      const semMister = platform !== "steam" && String(failure).includes("MISTER_GROOVY");
      const semMemoria = platform !== "steam" && String(failure).includes("MISTER_SEM_MEMORIA");
      setProblem({ kind: semEstado ? "ranked-needs-update" : semMemoria ? "mister-no-memory" : semMister ? "mister-groovy" : "launch-failed", retry });
    } finally {
      setIsLaunching(false);
    }
  }

  const dialogs: ReactNode = <>
    {linking && (
      <div className="controller-dialog-backdrop">
        <div ref={linkDialogRef} role="dialog" aria-modal="true" aria-labelledby="mister-link-title" tabIndex={-1} className="controller-dialog close-room-dialog">
          <header>
            <div>
              <h2 id="mister-link-title">{t("Qual MiSTer e qual jogador?")}</h2>
              <p>{t("Informe o IP do MiSTer e escolha quem é você na lista de jogadores dele. O Client lembra para as próximas vezes.")}</p>
            </div>
          </header>
          <div className="controller-dialog-body choose-platform-list">
            <form className="mister-link-form" onSubmit={(event) => { event.preventDefault(); void loadMisterPlayers(linking.host); }}>
              <label className="settings-field">{t("IP do MiSTer")}
                <input value={linking.host} onChange={(event) => setLinking({ ...linking, host: event.target.value })} placeholder="192.168.0.23" inputMode="decimal" autoComplete="off" required />
              </label>
              <button type="submit" className="secondary-action" disabled={linking.busy || !linking.host.trim()}>{t(linking.busy ? "Lendo o MiSTer…" : "Buscar jogadores")}</button>
            </form>
            {linking.error && <p className="mode-error-inline" role="alert">{t(linking.error)}</p>}
            {linking.players?.map((name) => (
              <button key={name} type="button" className="import-source-option" onClick={() => chooseMisterPlayer(name)}>
                <span className="platform-mark" aria-hidden="true">{name[0]?.toUpperCase()}</span>
                <span><strong>{name.toUpperCase()}</strong><small>{t("Abrir como este jogador")}</small></span>
              </button>
            ))}
          </div>
          <footer><span /><button type="button" className="secondary-action" onClick={() => setLinking(null)}>{t("Cancelar")}</button></footer>
        </div>
      </div>
    )}
    {misterModes && (
      <div className="controller-dialog-backdrop">
        <div ref={misterDialogRef} role="dialog" aria-modal="true" aria-labelledby="mister-mode-title" tabIndex={-1} className="controller-dialog close-room-dialog">
          <header>
            <div>
              <h2 id="mister-mode-title">{t("{game} no MiSTer", { game: room.title })}</h2>
              <p>{t("Escolha o modo. O jogo abre na TV, já com o seu jogador.")}</p>
            </div>
          </header>
          <div className="controller-dialog-body choose-platform-list">
            {misterModes.map((game) => (
              <button key={game.caminho} type="button" className="import-source-option" onClick={() => void openOnMister(game)}>
                <span className="platform-mark" aria-hidden="true">M</span>
                <span><strong>{game.modo}</strong><small>{game.jogo}</small></span>
              </button>
            ))}
          </div>
          <footer><span /><button type="button" className="secondary-action" onClick={() => setMisterModes(null)}>{t("Cancelar")}</button></footer>
        </div>
      </div>
    )}
    {problem && <LaunchProblemDialog kind={problem.kind} game={room.title} onRetry={problem.retry} onClose={() => setProblem(null)} />}
    {choosing && (
      <div className="controller-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) setChoosing(null); }}>
        <div ref={chooseDialogRef} role="dialog" aria-modal="true" aria-labelledby="choose-platform-title" tabIndex={-1} className="controller-dialog close-room-dialog play-dialog">
          <header>
            <div>
              <h2 id="choose-platform-title">{t(choosing.kind === "training" ? "Play Training · {game}" : choosing.kind === "live" ? "Play Live Ranked · {game}" : "Play Ranked · {game}", { game: room.title })}</h2>
              <p>{t(choosing.kind === "training"
                ? "Treino: o jogo abre sem gravar run nem gastar ficha."
                : choosing.kind === "live"
                  ? "Ao vivo no seu canal do YouTube. Escolha o modo e a plataforma."
                  : "Valendo ranking: gasta 1 ficha. Escolha o modo e a plataforma.")}</p>
            </div>
          </header>
          <div className="controller-dialog-body play-dialog-body">
            <h3 className="play-dialog-label">{t("Modo de jogo")}</h3>
            <div className="play-mode-options" role="radiogroup" aria-label={t("Modo de jogo")}>
              {playModesFor(room).map((m) => (
                <button key={m.id} type="button" role="radio" aria-checked={choosing.mode === m.id} className="play-mode-option" disabled={m.disabled} onClick={() => setChoosing({ ...choosing, mode: m.id })}>
                  <strong>{m.label}{m.supported && m.soon && <span className="coming-soon-tag">{t("Em breve")}</span>}</strong><small>{m.supported ? t(m.detail) : t("Este jogo não tem o modo {mode}.", { mode: m.label })}</small>
                </button>
              ))}
            </div>
            <h3 className="play-dialog-label">{t("Plataforma")}</h3>
            <div className="choose-platform-list" role="radiogroup" aria-label={t("Plataforma")}>
              {PLAY_ON.filter((p) => choosing.platforms.includes(p.id)).map((p) => (
                <button key={p.id} type="button" role="radio" aria-checked={choosing.platform === p.id} className="import-source-option" onClick={() => setChoosing({ ...choosing, platform: p.id })}>
                  <span className="platform-mark" aria-hidden="true">{p.label[0]}</span>
                  <span><strong>{t(p.label)}</strong><small>{t(p.id === "steam" ? "Abre pela Steam deste computador" : p.id === "emulator" ? "Abre no ShmupX Emulator deste computador" : choosing.kind === "training" ? "Abre no MiSTer como CONVIDADO (sem ficha nem ranking online)" : "Abre no MiSTer pela rede, já com o seu jogador")}</small></span>
                  <span className={`import-availability ${playable.owns(room, p.id) ? "is-available" : ""}`}>{t(playable.owns(room, p.id) ? "Na sua biblioteca" : "Não importado")}</span>
                </button>
              ))}
            </div>
          </div>
          <footer>
            <button type="button" className="secondary-action" onClick={() => setChoosing(null)}>{t("Cancelar")}</button>
            <button type="button" className="primary-action" onClick={confirmPlay}>{t("Jogar")}</button>
          </footer>
        </div>
      </div>
    )}
  </>;

  const banners: ReactNode = <>
    {error && <p className="room-banner warning" role="alert">{t(error)}</p>}
    {notice && <p className="room-banner" role="status">{notice}</p>}
  </>;

  return {
    start, play, playNow, isLaunching, isStopping, practice, setError, dialogs, banners, misterGame, stopMister, ensureInstalled,
    /** Plataformas em que dá para abrir o jogo por aqui, e se o jogador já tem o jogo nela. */
    platforms: options(),
    owns: (platform: PlayOn) => playable.owns(room, platform),
  };
}
