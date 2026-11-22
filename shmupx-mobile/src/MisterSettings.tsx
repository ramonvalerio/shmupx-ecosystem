import { useState } from "react";
import { useI18n } from "@client/i18n";
import MisterConnect from "@client/components/MisterConnect";
import { misterMatches, readMisterLibrary, saveMisterLibrary, scanMister, useMisterLibrary } from "@client/data/misterLibrary";
import { readMisterPlayer } from "@client/data/misterPlayer";
import type { GameRoom } from "@client/types/room";

/**
 * Ajustes do Lite: o MiSTer na mesma rede do celular. Ler os jogos (para saber quais salas dá para
 * jogar nele) e ligar o jogador à conta (instala/atualiza o ShmupX no MiSTer por SSH).
 */
export default function MisterSettings({ rooms, onBack }: { rooms: GameRoom[]; onBack: () => void }) {
  const { t } = useI18n();
  const library = useMisterLibrary();
  const player = readMisterPlayer();
  const [host, setHost] = useState(() => readMisterLibrary()?.host ?? player?.host ?? "");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const compatible = library.library ? rooms.filter((room) => misterMatches(room, library.library!)) : [];

  async function read() {
    setBusy(true);
    setError(null);
    try {
      const result = await scanMister(host, password);
      saveMisterLibrary({ host: result.host, mras: result.mras, roms: result.roms, importedAt: Date.now() });
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="lite-screen">
      <header className="lite-topbar">
        <button type="button" className="lite-icon-button" onClick={onBack} aria-label={t("Voltar")}>‹</button>
        <h1 className="lite-room-title">{t("Ajustes")}</h1>
      </header>
      <div className="lite-content lite-settings">
        <section className="lite-card">
          <h2>{t("MiSTer FPGA")}</h2>
          <p className="lite-muted">{t("Com o celular no mesmo Wi-Fi do MiSTer, dá para abrir os jogos nele pela sala, valendo ranking ou em treino.")}</p>
          <label className="settings-field">{t("IP do MiSTer")}
            <input value={host} onChange={(event) => setHost(event.target.value)} placeholder="192.168.0.23" inputMode="decimal" autoComplete="off" />
          </label>
          <label className="settings-field">{t("Senha do root (vazio = padrão)")}
            <input type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="off" />
          </label>
          <button type="button" className="primary-action" disabled={busy || !host.trim()} onClick={() => void read()}>
            {t(busy ? "Lendo os jogos do MiSTer…" : "Ler jogos do MiSTer")}
          </button>
          {error && <p className="mode-error-inline" role="alert">{t(error)}</p>}
          {library.library && (
            <p className="lite-muted">
              {t("{mras} jogos de arcade e {roms} ROMs no MiSTer {host}.", { mras: library.library.mras.length, roms: library.library.roms.length, host: library.library.host })}{" "}
              {compatible.length > 0 && t("Com sala no ShmupX: {games}.", { games: compatible.map((room) => room.title).join(", ") })}
            </p>
          )}
        </section>

        <section className="lite-card">
          <h2>{t("Partidas do MiSTer no ranking")}</h2>
          {player && <p className="lite-muted">{t("Ligado como {player} no MiSTer {host}.", { player: player.username.toUpperCase(), host: player.host })}</p>}
          <MisterConnect host={host.trim() || undefined} password={password} />
        </section>
      </div>
    </main>
  );
}
