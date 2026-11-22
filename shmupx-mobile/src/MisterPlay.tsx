import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "@client/i18n";
import { misterMatches, useMisterLibrary } from "@client/data/misterLibrary";
import { readMisterPlayer, type MisterMenuGame } from "@client/data/misterPlayer";
import type { GameRoom } from "@client/types/room";

const slug = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
const describe = (error: unknown) => (error instanceof Error ? error.message : String(error));

/** O jogo roda no MiSTer? (Lab marcou MiSTer ou a biblioteca lida do MiSTer tem o jogo.) */
export function useMisterCompatible(room: GameRoom) {
  const library = useMisterLibrary();
  return room.platforms.includes("mister") || Boolean(library.library && misterMatches(room, library.library));
}

/**
 * Jogar no MiSTer pelo celular: valendo ranking (com o jogador ligado à conta, gasta 1 ficha) ou
 * treino (como CONVIDADO, sem ficha nem ranking online). Escolhe o modo, abre na TV e PARAR JOGO
 * volta o MiSTer ao menu (o agente grava a partida com o placar).
 */
export default function MisterPlay({ room, onSettings }: { room: GameRoom; onSettings: () => void }) {
  const { t } = useI18n();
  const player = readMisterPlayer();
  const [practice, setPractice] = useState(false);
  const [modes, setModes] = useState<MisterMenuGame[] | null>(null);
  const [playing, setPlaying] = useState<MisterMenuGame | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!player) {
    return (
      <div className="lite-mister">
        <p className="lite-muted">{t("Este jogo roda no MiSTer. Ligue seu MiSTer em Ajustes para jogar por aqui.")}</p>
        <button type="button" className="lite-secondary" onClick={onSettings}>{t("Configurar MiSTer")}</button>
      </div>
    );
  }
  const host = player.host;
  const senha = player.password ?? null;

  async function loadModes() {
    setBusy(true);
    setError(null);
    try {
      const games = await invoke<MisterMenuGame[]>("mister_jogos_do_jogador", { host, senha, username: practice ? "convidado" : player!.username });
      const names = new Set([slug(room.title), slug(room.id)]);
      const list = games.filter((g) => names.has(slug(g.jogo)));
      if (list.length === 0) setError(t("{game} não está no menu ShmupX desse jogador no MiSTer. Conecte o MiSTer de novo em Ajustes.", { game: room.title }));
      setModes(list);
    } catch (failure) {
      setError(describe(failure));
    } finally {
      setBusy(false);
    }
  }

  async function start(game: MisterMenuGame) {
    setBusy(true);
    setError(null);
    try {
      await invoke("mister_iniciar", { host, senha, caminho: game.caminho });
      setPlaying(game);
      setModes(null);
    } catch (failure) {
      setError(describe(failure));
    } finally {
      setBusy(false);
    }
  }

  async function stop() {
    setBusy(true);
    setError(null);
    try {
      await invoke("mister_parar", { host, senha });
      setPlaying(null);
    } catch (failure) {
      setError(describe(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="lite-mister">
      {playing ? <>
        <p className="lite-mister-status">
          <span className="presence-dot is-live" aria-hidden="true" />
          {t(practice ? "Treinando na TV: {mode} (sem ficha)" : "Jogando na TV: {mode} (valendo ranking)", { mode: playing.modo })}
        </p>
        <button type="button" className="lite-stop" disabled={busy} onClick={() => void stop()}>{t(busy ? "Encerrando o jogo…" : "PARAR JOGO")}</button>
      </> : <>
        <div className="lite-segment" role="group" aria-label={t("Como jogar")}>
          <button type="button" aria-pressed={!practice} onClick={() => { setPractice(false); setModes(null); }}>{t("Valendo ranking")}</button>
          <button type="button" aria-pressed={practice} onClick={() => { setPractice(true); setModes(null); }}>{t("Treino")}</button>
        </div>
        <p className="lite-note">{t(practice
          ? "Treino: abre como CONVIDADO, sem gastar ficha nem ir para o ranking online."
          : "Valendo ranking: abre como {player} e gasta 1 ficha; o placar entra no seu perfil.", { player: player.username.toUpperCase() })}</p>
        {modes === null
          ? <button type="button" className="lite-primary" disabled={busy} onClick={() => void loadModes()}>{t(busy ? "Lendo o MiSTer…" : "Jogar no MiSTer")}</button>
          : <div className="lite-modes">
              {modes.map((game) => (
                <button key={game.caminho} type="button" className="lite-mode" disabled={busy} onClick={() => void start(game)}>{game.modo}</button>
              ))}
              <button type="button" className="lite-secondary" onClick={() => setModes(null)}>{t("Cancelar")}</button>
            </div>}
      </>}
      {error && <p className="lite-error" role="alert">{t(error)}</p>}
    </div>
  );
}
