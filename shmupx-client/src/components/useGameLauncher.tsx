import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import type { GameRoom, PlayOn } from "../types/room";
import { readEmulatorLibrary, romsFor } from "../data/emulatorLibrary";
import { readMisterPlayer, saveMisterPlayer, type MisterMenuGame } from "../data/misterPlayer";
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
export function useGameLauncher(room: GameRoom) {
  const { t } = useI18n();
  const playable = usePlatformOwnership();
  const [isLaunching, setIsLaunching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [practice, setPractice] = useState(false);
  /** Jogo aberto no MiSTer por aqui (o PARAR JOGO volta o MiSTer ao menu). */
  const [misterGame, setMisterGame] = useState<{ host: string; password?: string; practice: boolean } | null>(null);
  const [isStopping, setIsStopping] = useState(false);
  /** Plataformas para escolher (o jogador tem o jogo em mais de uma). */
  const [choosing, setChoosing] = useState<PlayOn[] | null>(null);
  const chooseDialogRef = useDialogFocus(choosing !== null, () => setChoosing(null));
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

  /** INICIAR JOGO (`forPractice` false) ou MODO TREINO: uma plataforma abre direto; mais de uma pergunta. */
  function start(forPractice: boolean) {
    setError(null);
    setNotice(null);
    setPractice(forPractice);
    const list = options();
    if (list.length === 0) {
      setError(room.platform && OWN_HARDWARE[room.platform]
          ? t("Este jogo roda no {platform}: abra-o no próprio aparelho.", { platform: OWN_HARDWARE[room.platform] })
          : t("O Client ainda não sabe abrir este jogo. Importe o jogo de uma plataforma em Importar jogos."));
      return;
    }
    if (list.length > 1) { setChoosing(list); return; }
    void launchOn(list[0], forPractice);
  }

  /** Abre o jogo no MiSTer pela rede (SSH): no menu ShmupX, com o jogador da conta e o modo. */
  async function openOnMister(game?: MisterMenuGame, forPractice = practice) {
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
        const games = await invoke<MisterMenuGame[]>("mister_jogos_do_jogador", { host: player.host, senha: player.password ?? null, username });
        const names = new Set([slug(room.title), slug(room.id)]);
        const modes = games.filter((g) => names.has(slug(g.jogo)));
        if (modes.length === 0) {
          setError(forPractice
            ? t("O jogador CONVIDADO do seu MiSTer não tem {game}. Conecte o MiSTer de novo em Importar jogos para atualizar o menu.", { game: room.title })
            : t("{game} não está no menu ShmupX do seu MiSTer. Conecte o MiSTer de novo em Importar jogos para atualizar o menu.", { game: room.title }));
          return;
        }
        if (modes.length > 1) { setMisterModes(modes); return; }
        target = modes[0];
      }
      setMisterModes(null);
      await invoke("mister_iniciar", { host: player.host, senha: player.password ?? null, caminho: target.caminho });
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
      const players = await invoke<string[]>("mister_usuarios", { host: host.trim(), senha: null });
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
      await invoke("mister_parar", { host: misterGame.host, senha: misterGame.password ?? null });
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

  async function launchOn(platform: PlayOn, forPractice = practice) {
    setChoosing(null);
    setError(null);
    setNotice(null);
    if (platform === "mister") {
      await openOnMister(undefined, forPractice);
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
                <span><strong>{t(p.label)}</strong><small>{t(p.id === "steam" ? "Abre pela Steam deste computador" : p.id === "emulator" ? "Abre no Fightcade (FBNeo) deste computador" : practice ? "Abre no MiSTer como CONVIDADO (sem ficha nem ranking online)" : "Abre no MiSTer pela rede, já com o seu jogador")}</small></span>
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

  return { start, isLaunching, isStopping, practice, setError, dialogs, banners, misterGame, stopMister };
}
