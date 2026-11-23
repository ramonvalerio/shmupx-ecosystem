import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import type { GameMode, GameRoom, PlayOn } from "../types/room";
import { GAME_MODES, misterEntryMatches, modesFor, setPendingRunMode } from "../data/gameModes";
import { readEmulatorLibrary, romsFor } from "../data/emulatorLibrary";
import { misterAuth, readMisterPlayer, saveMisterPlayer, type MisterMenuGame } from "../data/misterPlayer";
import { readMisterLibrary } from "../data/misterLibrary";
import { PLAY_ON, stillInstalled, usePlatformOwnership } from "../data/platforms";
import { useDialogFocus } from "./useDialogFocus";

/** Plataformas em que o jogo roda fora do PC: o Client não tem como abri-lo. */
const OWN_HARDWARE: Record<string, string> = { mister: "MiSTer", xbox360: "Xbox 360" };

/** Nome comparável (sem acento, minúsculas, hífens): o menu do MiSTer usa o título do jogo. */
const slug = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

const describe = (error: unknown, fallback: string) => typeof error === "string" ? error : error instanceof Error ? error.message : fallback;

/**
 * Abrir o jogo (INICIAR JOGO e MODO TREINO), usado pela sala e pelo painel da coleção: escolhe a
 * plataforma (Steam, emulador ou MiSTer, conforme as bibliotecas do jogador), confere se o jogo
 * continua instalado e abre. No treino, o jogo abre sem gravar run nem gastar ficha (o Rust avisa
 * a abertura com `treino`, e o App não inicia a corrida); o MiSTer fica de fora, porque o agente
 * dele registra toda partida aberta pelo menu do jogador.
 */
/** Play Training (sem ficha nem gravação), Play Ranked (gasta 1 ficha e grava) ou Play Live Ranked. */
export type PlayKind = "training" | "ranked" | "live";

export function useGameLauncher(room: GameRoom, { onLive }: { onLive?: (platform: PlayOn, mode: GameMode) => void } = {}) {
  const { t } = useI18n();
  const playable = usePlatformOwnership();
  const [isLaunching, setIsLaunching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [practice, setPractice] = useState(false);
  /** Jogo aberto no MiSTer por aqui (o PARAR JOGO volta o MiSTer ao menu). */
  const [misterGame, setMisterGame] = useState<{ host: string; password?: string; practice: boolean } | null>(null);
  const [isStopping, setIsStopping] = useState(false);
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
  /** Jogo desinstalado (ou sem o ROM) na plataforma escolhida. */
  const [notFound, setNotFound] = useState<PlayOn | null>(null);
  const notFoundDialogRef = useDialogFocus(notFound !== null, () => setNotFound(null));

  useEffect(() => {
    setError(null);
    setNotice(null);
    setChoosing(null);
    setMisterModes(null);
    setNotFound(null);
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

  /** Play Training / Play Ranked / Play Live Ranked: abre a janela de modo e plataforma. */
  function play(kind: PlayKind) {
    setError(null);
    setNotice(null);
    setPractice(kind === "training");
    // A live captura a janela do jogo neste PC: o MiSTer (na TV) fica de fora.
    const list = options().filter((id) => kind !== "live" || id !== "mister");
    if (list.length === 0) {
      setError(room.platform && OWN_HARDWARE[room.platform]
          ? t("Este jogo roda no {platform}: abra-o no próprio aparelho.", { platform: OWN_HARDWARE[room.platform] })
          : t("O Client ainda não sabe abrir este jogo. Importe o jogo de uma plataforma em Importar jogos."));
      return;
    }
    const owned = list.find((id) => playable.owns(room, id));
    setChoosing({ kind, platforms: list, mode: modesFor(room)[0], platform: owned ?? list[0] });
  }

  /**
   * Abre direto, com o modo e a plataforma já escolhidos no cabeçalho da sala (sem a janela).
   * Live: a sala põe a live no ar (o MiSTer fica de fora, porque a live captura a janela no PC).
   */
  function playNow(kind: PlayKind, platform: PlayOn, chosen: GameMode) {
    setError(null);
    setNotice(null);
    setPractice(kind === "training");
    setMode(chosen);
    if (kind === "live") { onLive?.(platform, chosen); return; }
    if (kind === "ranked" && platform !== "mister") setPendingRunMode(room.id, chosen);
    void launchOn(platform, kind === "training", chosen);
  }

  /** Compatível com as telas antigas: `start(true)` é o Play Training, `start(false)` o Play Ranked. */
  function start(forPractice: boolean) { play(forPractice ? "training" : "ranked"); }

  function confirmPlay() {
    if (!choosing) return;
    const { kind, platform, mode: chosen } = choosing;
    setChoosing(null);
    setMode(chosen);
    if (kind === "live") { onLive?.(platform, chosen); return; }
    // A run começa quando o jogo abrir (PC): leva o modo escolhido.
    if (kind === "ranked" && platform !== "mister") setPendingRunMode(room.id, chosen);
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
    try {
      let target = game;
      if (!target) {
        // Treino no MiSTer: pelo CONVIDADO (sem token), sem ficha e sem ranking online.
        const username = forPractice ? "convidado" : player.username;
        const games = await invoke<MisterMenuGame[]>("mister_jogos_do_jogador", { host: player.host, ...misterAuth(player.password), username });
        const names = new Set([slug(room.title), slug(room.id)]);
        const modes = games.filter((g) => names.has(slug(g.jogo)));
        if (modes.length === 0) {
          setError(forPractice
            ? t("O jogador CONVIDADO do seu MiSTer não tem {game}. Conecte o MiSTer de novo em Importar jogos para atualizar o menu.", { game: room.title })
            : t("{game} não está no menu ShmupX do seu MiSTer. Conecte o MiSTer de novo em Importar jogos para atualizar o menu.", { game: room.title }));
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
      setNotice(forPractice
        ? t("Modo treino: abrindo {game} no MiSTer ({mode}) como CONVIDADO, sem gastar ficha nem ir para o ranking online.", { game: room.title, mode: target.modo })
        : t("Abrindo {game} no MiSTer ({mode}) como {player}. Jogue na TV: a partida entra no seu perfil.", { game: room.title, mode: target.modo, player: player.username.toUpperCase() }));
    } catch (failure) {
      setError(describe(failure, "Falha ao abrir o jogo no MiSTer."));
    } finally {
      setIsLaunching(false);
    }
  }

  async function loadMisterPlayers(host: string) {
    setLinking({ host, players: null, busy: true, error: null });
    try {
      const players = await invoke<string[]>("mister_usuarios", { host: host.trim(), ...misterAuth() });
      setLinking({ host, players, busy: false, error: players.length ? null : t("Nenhum jogador no shmupx.ini desse MiSTer. Use Conectar ao meu MiSTer em Importar jogos para instalar o ShmupX nele.") });
    } catch (failure) {
      setLinking({ host, players: null, busy: false, error: describe(failure, "Não foi possível ler o MiSTer.") });
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
    } catch (failure) {
      setError(describe(failure, "Não foi possível parar o jogo no MiSTer."));
    } finally {
      setIsStopping(false);
    }
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
    try {
      if (!(await stillInstalled(room, platform))) { setNotFound(platform); return; }
      if (platform === "steam") {
        if (room.steamAppId === 0) throw "Este jogo não tem app_id da Steam no catálogo.";
        await invoke("inicializar_jogo_steam", { steamAppId: room.steamAppId, treino: forPractice });
      } else {
        await invoke("inicializar_jogo_emulador", { jogoId: room.id, roms: romsFor(room), pasta: readEmulatorLibrary()?.root || null, treino: forPractice });
      }
      if (forPractice) setNotice(t("Modo treino: {game} abre sem gravar run nem gastar ficha.", { game: room.title }));
    } catch (failure) {
      setError(describe(failure, "Falha ao iniciar o jogo."));
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
    {notFound && (
      <div className="controller-dialog-backdrop">
        <div ref={notFoundDialogRef} role="alertdialog" aria-modal="true" aria-labelledby="not-found-title" tabIndex={-1} className="controller-dialog close-room-dialog">
          <header>
            <div>
              <h2 id="not-found-title">{t("Jogo não encontrado")}</h2>
              <p>{notFound === "steam"
                ? t("{game} não está instalado na Steam deste computador. Instale o jogo na Steam e tente de novo.", { game: room.title })
                : t("O ROM de {game} não está no Fightcade deste computador. Baixe o jogo pelo Fightcade e tente de novo.", { game: room.title })}</p>
            </div>
          </header>
          <footer><span /><button type="button" className="primary-action" onClick={() => setNotFound(null)}>{t("Entendi")}</button></footer>
        </div>
      </div>
    )}
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
                  : "Valendo ranking: gasta 1 ficha e grava a run. Escolha o modo e a plataforma.")}</p>
            </div>
          </header>
          <div className="controller-dialog-body play-dialog-body">
            <h3 className="play-dialog-label">{t("Modo de jogo")}</h3>
            <div className="play-mode-options" role="radiogroup" aria-label={t("Modo de jogo")}>
              {GAME_MODES.filter((m) => modesFor(room).includes(m.id)).map((m) => (
                <button key={m.id} type="button" role="radio" aria-checked={choosing.mode === m.id} className="play-mode-option" onClick={() => setChoosing({ ...choosing, mode: m.id })}>
                  <strong>{m.label}</strong><small>{t(m.detail)}</small>
                </button>
              ))}
            </div>
            <h3 className="play-dialog-label">{t("Plataforma")}</h3>
            <div className="choose-platform-list" role="radiogroup" aria-label={t("Plataforma")}>
              {PLAY_ON.filter((p) => choosing.platforms.includes(p.id)).map((p) => (
                <button key={p.id} type="button" role="radio" aria-checked={choosing.platform === p.id} className="import-source-option" onClick={() => setChoosing({ ...choosing, platform: p.id })}>
                  <span className="platform-mark" aria-hidden="true">{p.label[0]}</span>
                  <span><strong>{t(p.label)}</strong><small>{t(p.id === "steam" ? "Abre pela Steam deste computador" : p.id === "emulator" ? "Abre no Fightcade (FBNeo) deste computador" : choosing.kind === "training" ? "Abre no MiSTer como CONVIDADO (sem ficha nem ranking online)" : "Abre no MiSTer pela rede, já com o seu jogador")}</small></span>
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
    start, play, playNow, isLaunching, isStopping, practice, setError, dialogs, banners, misterGame, stopMister,
    /** Plataformas em que dá para abrir o jogo por aqui, e se o jogador já tem o jogo nela. */
    platforms: options(),
    owns: (platform: PlayOn) => playable.owns(room, platform),
  };
}
