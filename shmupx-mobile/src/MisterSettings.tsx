import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "@client/i18n";
import MisterConnect from "@client/components/MisterConnect";
import { misterMatches, readMisterLibrary, saveMisterLibrary, scanMister, useMisterLibrary } from "@client/data/misterLibrary";
import { misterAuth, readMisterAccess, readMisterPlayer, saveMisterAccess } from "@client/data/misterPlayer";
import type { GameRoom } from "@client/types/room";
import type { Language } from "@client/i18nMessages";
import { THEMES, saveLanguage, type Theme } from "./prefs";

const describe = (error: unknown) => (error instanceof Error ? error.message : String(error));

/**
 * Ajustes do Lite: o MiSTer na mesma rede Wi-Fi. Sem IP salvo, procura o MiSTer sozinho com o
 * acesso de fábrica (root / 1) e já lê os jogos; usuário e senha só precisam ser trocados se o
 * MiSTer não usa os padrão. Depois, ligar o jogador à conta (instala o ShmupX no MiSTer).
 */
export default function MisterSettings({ rooms, theme, onTheme, onBack }: { rooms: GameRoom[]; theme: Theme; onTheme: (next: Theme) => void; onBack: () => void }) {
  const { t, language, setLanguage } = useI18n();
  const library = useMisterLibrary();
  const player = readMisterPlayer();
  const [host, setHost] = useState(() => readMisterLibrary()?.host ?? player?.host ?? "");
  const [user, setUser] = useState(() => readMisterAccess().user ?? "");
  const [password, setPassword] = useState(() => readMisterAccess().password ?? "");
  const [status, setStatus] = useState<"idle" | "searching" | "reading">("idle");
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const searched = useRef(false);
  const compatible = library.library ? rooms.filter((room) => misterMatches(room, library.library!)) : [];
  const customAccess = Boolean(readMisterAccess().user || readMisterAccess().password);

  function saveAccess(nextUser = user, nextPassword = password) {
    saveMisterAccess({ user: nextUser, password: nextPassword });
  }

  async function read(target = host) {
    if (!target.trim()) return;
    setStatus("reading");
    setError(null);
    try {
      saveAccess();
      const result = await scanMister(target.trim());
      saveMisterLibrary({ host: result.host, mras: result.mras, roms: result.roms, importedAt: Date.now() });
      setNotice(null);
    } catch (failure) {
      setError(describe(failure));
    } finally {
      setStatus("idle");
    }
  }

  /** Procura o MiSTer na rede do celular e, achando, já lê os jogos. */
  async function search() {
    setStatus("searching");
    setError(null);
    setNotice(null);
    try {
      saveAccess();
      const { usuario, senha } = misterAuth();
      const found = await invoke<string[]>("mister_procurar", { usuario, senha });
      if (found.length === 0) {
        setStatus("idle");
        setError(t("Nenhum MiSTer encontrado nesta rede. Confira se o MiSTer está ligado no mesmo Wi-Fi; se o usuário ou a senha não forem os de fábrica, ajuste abaixo. Também dá para digitar o IP."));
        return;
      }
      setHost(found[0]);
      if (found.length > 1) setNotice(t("Mais de um MiSTer na rede: usando {host}. Troque o IP se for outro.", { host: found[0] }));
      await read(found[0]);
    } catch (failure) {
      setStatus("idle");
      setError(describe(failure));
    }
  }

  // Primeira vez (sem IP): procura sozinho.
  useEffect(() => {
    if (searched.current || host) return;
    searched.current = true;
    void search();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const busy = status !== "idle";

  return (
    <main className="lite-screen">
      <header className="lite-topbar">
        <button type="button" className="lite-icon-button" onClick={onBack} aria-label={t("Voltar")}>‹</button>
        <h1 className="lite-room-title">{t("Ajustes")}</h1>
      </header>
      <div className="lite-content lite-settings">
        <section className="lite-card">
          <h2>{t("Interface")}</h2>
          <h3 className="lite-sheet-label">{t("Tema")}</h3>
          <div className="lite-preset-row" role="radiogroup" aria-label={t("Tema")}>
            {THEMES.map((item) => (
              <button key={item.id} type="button" role="radio" aria-checked={theme === item.id} onClick={() => onTheme(item.id)}>
                <span className={`lite-swatch is-${item.id}`} aria-hidden="true" />{t(item.label)}
              </button>
            ))}
          </div>
          <h3 className="lite-sheet-label">{t("Idioma")}</h3>
          <div className="lite-preset-row" role="radiogroup" aria-label={t("Idioma")}>
            {([{ id: "pt-BR", label: "Português" }, { id: "en", label: "English" }] as { id: Language; label: string }[]).map((item) => (
              <button key={item.id} type="button" role="radio" aria-checked={language === item.id} onClick={() => { setLanguage(item.id); saveLanguage(item.id); }}>{item.label}</button>
            ))}
          </div>
        </section>
        <section className="lite-card">
          <h2>{t("MiSTer FPGA")}</h2>
          <p className="lite-muted">{t("Com o celular no mesmo Wi-Fi do MiSTer, dá para abrir os jogos nele pela sala, valendo ranking ou em treino.")}</p>

          {status === "searching" && <p className="lite-status" role="status"><span className="lite-spinner" aria-hidden="true" />{t("Procurando o MiSTer na rede…")}</p>}
          {status === "reading" && <p className="lite-status" role="status"><span className="lite-spinner" aria-hidden="true" />{t("Lendo os jogos do MiSTer…")}</p>}
          {library.library && !busy && (
            <p className="lite-found">
              <span className="presence-dot is-online" aria-hidden="true" />
              <span>{t("MiSTer em {host}: {mras} jogos de arcade e {roms} ROMs.", { host: library.library.host, mras: library.library.mras.length, roms: library.library.roms.length })}
                {compatible.length > 0 && <> {t("Com sala no ShmupX: {games}.", { games: compatible.map((room) => room.title).join(", ") })}</>}</span>
            </p>
          )}

          <label className="settings-field">{t("IP do MiSTer")}
            <input value={host} onChange={(event) => setHost(event.target.value)} placeholder="192.168.0.23" inputMode="decimal" autoComplete="off" />
          </label>
          <div className="lite-actions">
            <button type="button" className="primary-action" disabled={busy || !host.trim()} onClick={() => void read()}>{t("Ler jogos do MiSTer")}</button>
            <button type="button" className="secondary-action" disabled={busy} onClick={() => void search()}>{t("Procurar na rede")}</button>
          </div>

          <details className="lite-access" open={customAccess || undefined}>
            <summary>{t("Usuário e senha do MiSTer")} <small>{t(customAccess ? "personalizados" : "padrão: root / 1")}</small></summary>
            <label className="settings-field">{t("Usuário")}
              <input value={user} onChange={(event) => { setUser(event.target.value); saveAccess(event.target.value, password); }} placeholder="root" autoComplete="off" autoCapitalize="none" />
            </label>
            <label className="settings-field">{t("Senha")}
              <input type="password" value={password} onChange={(event) => { setPassword(event.target.value); saveAccess(user, event.target.value); }} placeholder="1" autoComplete="off" />
            </label>
            <p className="lite-muted">{t("Deixe em branco para usar o acesso de fábrica do MiSTer.")}</p>
          </details>

          {notice && <p className="lite-muted" role="status">{notice}</p>}
          {error && <p className="mode-error-inline" role="alert">{t(error)}</p>}
        </section>

        <section className="lite-card">
          <h2>{t("Partidas do MiSTer no ranking")}</h2>
          {player
            ? <p className="lite-found"><span className="presence-dot is-online" aria-hidden="true" /><span>{t("Ligado como {player} no MiSTer {host}.", { player: player.username.toUpperCase(), host: player.host })}</span></p>
            : <p className="lite-muted">{t("Falta ligar o seu jogador do MiSTer à conta: escolha quem você é no MiSTer e o ShmupX é instalado nele.")}</p>}
          {host.trim()
            ? <MisterConnect key={host.trim()} host={host.trim()} password={password} />
            : <p className="lite-muted">{t("Encontre o MiSTer acima para continuar.")}</p>}
        </section>
      </div>
    </main>
  );
}
