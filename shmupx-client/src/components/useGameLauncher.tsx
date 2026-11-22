import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import type { GameRoom, PlayOn } from "../types/room";
import { readEmulatorLibrary, romsFor } from "../data/emulatorLibrary";
import { readMisterPlayer, type MisterMenuGame } from "../data/misterPlayer";
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
export function useGameLauncher(room: GameRoom) {
  const { t } = useI18n();
  const playable = usePlatformOwnership();
  const [isLaunching, setIsLaunching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [practice, setPractice] = useState(false);
  /** Plataformas para escolher (o jogador tem o jogo em mais de uma). */
  const [choosing, setChoosing] = useState<PlayOn[] | null>(null);
  const chooseDialogRef = useDialogFocus(choosing !== null, () => setChoosing(null));
  /** Modos do jogo no menu ShmupX do MiSTer, para escolher qual abrir na TV. */
  const [misterModes, setMisterModes] = useState<MisterMenuGame[] | null>(null);
  const misterDialogRef = useDialogFocus(misterModes !== null, () => setMisterModes(null));
  /** Jogo desinstalado (ou sem o ROM) na plataforma escolhida. */
  const [notFound, setNotFound] = useState<PlayOn | null>(null);
  const notFoundDialogRef = useDialogFocus(notFound !== null, () => setNotFound(null));

  useEffect(() => {
    setError(null);
    setNotice(null);
    setChoosing(null);
    setMisterModes(null);
    setNotFound(null);
    setIsLaunching(false);
  }, [room.id]);

  /**
   * Por onde dá para abrir: todas as plataformas em que o jogo roda (as do Lab e as em que o
   * jogador já tem o jogo) e o que o Client sabe abrir (Steam pelo app_id, emulador pelo ROM).
   * Com mais de uma, o jogador escolhe; a janela mostra em quais ele já tem o jogo.
   */
  function options(forPractice: boolean): PlayOn[] {
    const ids = new Set<PlayOn>(playable.tags(room).map((tag) => tag.id));
    if (room.steamAppId > 0) ids.add("steam");
    if (romsFor(room).length > 0) ids.add("emulator");
    const list = PLAY_ON.map((p) => p.id).filter((id) => ids.has(id));
    return forPractice ? list.filter((id) => id !== "mister") : list;
  }

  /** INICIAR JOGO (`forPractice` false) ou MODO TREINO: uma plataforma abre direto; mais de uma pergunta. */
  function start(forPractice: boolean) {
    setError(null);
    setNotice(null);
    setPractice(forPractice);
    const list = options(forPractice);
    if (list.length === 0) {
      setError(forPractice && options(false).includes("mister")
        ? t("O modo treino ainda não está disponível no MiSTer: lá toda partida aberta pelo menu do jogador é registrada.")
        : room.platform && OWN_HARDWARE[room.platform]
          ? t("Este jogo roda no {platform}: abra-o no próprio aparelho.", { platform: OWN_HARDWARE[room.platform] })
          : t("O Client ainda não sabe abrir este jogo. Importe o jogo de uma plataforma em Importar jogos."));
      return;
    }
    if (list.length > 1) { setChoosing(list); return; }
    void launchOn(list[0], forPractice);
  }

  /** Abre o jogo no MiSTer pela rede (SSH): no menu ShmupX, com o jogador da conta e o modo. */
  async function openOnMister(game?: MisterMenuGame) {
    const player = readMisterPlayer();
    if (!player) {
      setError(t("Ligue seu MiSTer à conta em Importar jogos › MiSTer FPGA (Conectar ao meu MiSTer) para abrir o jogo nele."));
      return;
    }
    setIsLaunching(true);
    try {
      let target = game;
      if (!target) {
        const games = await invoke<MisterMenuGame[]>("mister_jogos_do_jogador", { host: player.host, senha: player.password ?? null, username: player.username });
        const names = new Set([slug(room.title), slug(room.id)]);
        const modes = games.filter((g) => names.has(slug(g.jogo)));
        if (modes.length === 0) {
          setError(t("{game} não está no menu ShmupX do seu MiSTer. Conecte o MiSTer de novo em Importar jogos para atualizar o menu.", { game: room.title }));
          return;
        }
        if (modes.length > 1) { setMisterModes(modes); return; }
        target = modes[0];
      }
      setMisterModes(null);
      await invoke("mister_iniciar", { host: player.host, senha: player.password ?? null, caminho: target.caminho });
      setNotice(t("Abrindo {game} no MiSTer ({mode}) como {player}. Jogue na TV: a partida entra no seu perfil.", { game: room.title, mode: target.modo, player: player.username.toUpperCase() }));
    } catch (failure) {
      setError(describe(failure, "Falha ao abrir o jogo no MiSTer."));
    } finally {
      setIsLaunching(false);
    }
  }

  async function launchOn(platform: PlayOn, forPractice = practice) {
    setChoosing(null);
    setError(null);
    setNotice(null);
    if (platform === "mister") {
      await openOnMister();
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
      <div className="controller-dialog-backdrop">
        <div ref={chooseDialogRef} role="dialog" aria-modal="true" aria-labelledby="choose-platform-title" tabIndex={-1} className="controller-dialog close-room-dialog">
          <header>
            <div>
              <h2 id="choose-platform-title">{t(practice ? "Treinar {game}" : "Iniciar {game}", { game: room.title })}</h2>
              <p>{t(practice ? "Modo treino: o jogo abre sem gravar run nem gastar ficha. Por qual plataforma?" : "Este jogo roda em mais de uma plataforma. Por qual quer jogar?")}</p>
            </div>
          </header>
          <div className="controller-dialog-body choose-platform-list">
            {PLAY_ON.filter((p) => choosing.includes(p.id)).map((p) => (
              <button key={p.id} type="button" className="import-source-option" onClick={() => void launchOn(p.id)}>
                <span className="platform-mark" aria-hidden="true">{p.label[0]}</span>
                <span><strong>{t(p.label)}</strong><small>{t(p.id === "steam" ? "Abre pela Steam deste computador" : p.id === "emulator" ? "Abre no Fightcade (FBNeo) deste computador" : "Abre no MiSTer pela rede, já com o seu jogador")}</small></span>
                <span className={`import-availability ${playable.owns(room, p.id) ? "is-available" : ""}`}>{t(playable.owns(room, p.id) ? "Na sua biblioteca" : "Não importado")}</span>
              </button>
            ))}
          </div>
          <footer><span /><button type="button" className="secondary-action" onClick={() => setChoosing(null)}>{t("Cancelar")}</button></footer>
        </div>
      </div>
    )}
  </>;

  const banners: ReactNode = <>
    {error && <p className="room-banner warning" role="alert">{t(error)}</p>}
    {notice && <p className="room-banner" role="status">{notice}</p>}
  </>;

  return { start, isLaunching, practice, setError, dialogs, banners };
}
