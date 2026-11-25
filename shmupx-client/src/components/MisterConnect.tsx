import { useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../i18n";
import type { GameRoom } from "../types/room";
import { misterMatches, readMisterLibrary, saveMisterLibrary, scanMister, syncMisterMenu, type MisterLibrary } from "../data/misterLibrary";
import { misterAuth, readMisterAccess, readMisterPlayer, saveMisterAccess, saveMisterPlayer } from "../data/misterPlayer";

type Step = "idle" | "loading" | "choose" | "saving" | "done" | "error";

/**
 * Conecta o MiSTer FPGA (ADR 008), num formulário só: IP, usuário e senha uma vez. Conectar já é
 * para jogar valendo ranking: com `rooms`, o Client lê os jogos do MiSTer (arcade, Mega Drive e Neo
 * Geo) e marca na coleção os que têm sala; depois o jogador escolhe quem é no `shmupx.ini`, o
 * ShmupX é instalado no MiSTer com um token novo (que não aparece na tela) e os jogos do Lab que ele
 * tem entram no menu ShmupX. Alternativa: receber o token por e-mail e colar à mão.
 * `host`/`password`: os de uma tela que já achou o MiSTer (Lite); sem eles, pede aqui.
 * Usuário e senha vazios usam os de fábrica do MiSTer (`root` / `1`).
 */
export default function MisterConnect({ host: hostProp, password: passwordProp, rooms, tutorial, onImported }: {
  host?: string;
  password?: string;
  /** Catálogo: com ele, conectar também lê os jogos do MiSTer e sincroniza o menu ShmupX. */
  rooms?: GameRoom[];
  /** Marca o botão principal para o tutorial (Importar jogos). */
  tutorial?: boolean;
  /** A biblioteca do MiSTer foi lida e guardada. */
  onImported?: () => void;
}) {
  const { t } = useI18n();
  const [library, setLibrary] = useState<MisterLibrary | null>(null);
  const [menu, setMenu] = useState<{ added?: string[]; error?: string } | null>(null);
  const [host, setHost] = useState(() => hostProp ?? readMisterPlayer()?.host ?? readMisterLibrary()?.host ?? "");
  const [password, setPassword] = useState(passwordProp ?? "");
  const [user, setUser] = useState(() => readMisterAccess().user ?? "");
  const [step, setStep] = useState<Step>("idle");
  const [players, setPlayers] = useState<string[]>([]);
  const [player, setPlayer] = useState("");
  const [newPlayer, setNewPlayer] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [progress, setProgress] = useState<string | null>(null);
  const [mail, setMail] = useState<{ kind: "idle" | "sending" | "sent" | "error"; text?: string }>({ kind: "idle" });
  const target = hostProp ?? host;
  const secret = passwordProp ?? password;
  const chosen = player === "__new" ? newPlayer.trim().toLowerCase() : player;

  async function loadPlayers() {
    if (!isTauri()) { setStep("error"); setMessage(t("Entre com sua conta para conectar o MiSTer.")); return; }
    setStep("loading");
    setMessage(null);
    setMenu(null);
    try {
      if (hostProp === undefined) saveMisterAccess({ user, password: secret || readMisterAccess().password });
      if (rooms) {
        // Os jogos do MiSTer entram na coleção já nesta etapa.
        const scan = await scanMister(target, secret || undefined);
        const next = { ...scan, importedAt: Date.now() };
        saveMisterLibrary(next);
        setLibrary(next);
        onImported?.();
      }
      const list = await invoke<string[]>("mister_usuarios", { host: target, ...misterAuth(secret) });
      setPlayers(list);
      setPlayer(list[0] ?? "__new");
      setStep("choose");
    } catch (error) {
      setStep("error");
      setMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function connect() {
    setStep("saving");
    setMessage(null);
    // Andamento da instalação (conectar, baixar, copiar, montar o menu).
    const stop = await listen<string>("mister://etapa", (event) => setProgress(event.payload));
    try {
      await invoke<string>("mister_conectar", { host: target, ...misterAuth(secret), username: chosen });
      // INICIAR JOGO › MiSTer usa este jogador e este MiSTer.
      saveMisterPlayer({ host: target.trim(), username: chosen, password: secret || undefined });
      setStep("done");
      setMessage(t("Pronto: o ShmupX está instalado no MiSTer e {player} está ligado à sua conta. No MiSTer, abra a pasta ShmupX no menu principal; as partidas desse jogador entram no seu perfil e no ranking.", { player: chosen.toUpperCase() }));
      // Jogos do Lab que o MiSTer tem (arcade, Mega Drive, Neo Geo) entram no menu ShmupX dele.
      if (rooms && library) {
        syncMisterMenu(target.trim(), secret || undefined, rooms, library)
          .then((added) => setMenu({ added }))
          .catch((error) => setMenu({ error: error instanceof Error ? error.message : String(error) }));
      }
    } catch (error) {
      setStep("error");
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      stop();
      setProgress(null);
    }
  }

  async function sendByEmail() {
    if (!isTauri()) { setMail({ kind: "error", text: t("Entre com sua conta para conectar o MiSTer.") }); return; }
    setMail({ kind: "sending" });
    try {
      const reply = await invoke<{ enviado_para: string }>("mister_enviar_token");
      setMail({ kind: "sent", text: t("Token enviado para {email}. Cole a linha do e-mail no shmupx.ini do MiSTer.", { email: reply.enviado_para }) });
    } catch (error) {
      setMail({ kind: "error", text: error instanceof Error ? error.message : String(error) });
    }
  }

  return <div className="mister-connect">
    {hostProp === undefined && <div className="mister-connect-form">
      <label className="settings-field">{t("IP do MiSTer")}<input value={host} onChange={(event) => setHost(event.target.value)} placeholder="192.168.0.23" inputMode="decimal" autoComplete="off" /></label>
      <label className="settings-field">{t("Usuário do MiSTer (vazio = root)")}<input value={user} onChange={(event) => setUser(event.target.value)} placeholder="root" autoComplete="off" autoCapitalize="none" /></label>
      <label className="settings-field">{t("Senha (vazio = padrão 1)")}<input type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="off" /></label>
    </div>}
    {(step === "idle" || step === "loading" || step === "error") && (
      <button className="primary-action" data-tutorial={tutorial ? "import-run" : undefined} disabled={step === "loading" || !target.trim()} onClick={() => void loadPlayers()}>
        {t(step === "loading" ? "Lendo o MiSTer…" : "Conectar ao meu MiSTer")}
      </button>
    )}
    {rooms && library && step !== "error" && (() => {
      const found = rooms.filter((room) => misterMatches(room, library));
      return found.length > 0
        ? <><p className="muted" role="status">{t("{count} jogos do seu MiSTer têm sala no ShmupX:", { count: found.length })}</p><ul className="import-found-list">{found.map((room) => <li key={room.id}><span className="supported">✓</span> {room.title}</li>)}</ul></>
        : <p className="muted" role="status">{t("Nenhum jogo do MiSTer tem sala no ShmupX ainda.")}</p>;
    })()}
    {(step === "choose" || step === "saving") && <>
      <label className="settings-field">{t("Qual jogador do MiSTer é você?")}
        <select value={player} onChange={(event) => setPlayer(event.target.value)}>
          {players.map((name) => <option key={name} value={name}>{name.toUpperCase()}</option>)}
          <option value="__new">{t("Novo jogador…")}</option>
        </select>
      </label>
      {player === "__new" && <label className="settings-field">{t("Nome do jogador no MiSTer")}
        <input value={newPlayer} onChange={(event) => setNewPlayer(event.target.value.replace(/[^A-Za-z0-9_-]/g, "").slice(0, 12))} placeholder="ramonvalerio" maxLength={12} autoComplete="off" />
      </label>}
      <button className="primary-action" data-tutorial={tutorial ? "import-run" : undefined} disabled={step === "saving" || !chosen} onClick={() => void connect()}>
        {t(step === "saving" ? "Instalando no MiSTer…" : "Instalar e ligar à minha conta")}
      </button>
      {progress && <p className="muted" role="status">{t(progress)}</p>}
    </>}
    {message && <p className={step === "error" ? "mode-error" : "success-text"} role="status">{t(message)}</p>}
    {menu?.added && <p className="muted" role="status">{menu.added.length > 0
      ? t("Menu ShmupX do MiSTer atualizado: {games}.", { games: menu.added.join(", ") })
      : t("O menu ShmupX do MiSTer já tem todos os jogos do ShmupX que ele tem.")}</p>}
    {menu?.error && <p className="mode-error" role="alert">{t("Não foi possível atualizar o menu ShmupX do MiSTer: {reason}", { reason: menu.error })}</p>}
    <p className="muted">{t("Ao conectar, o ShmupX lê os jogos do MiSTer, instala o menu ShmupX nele e liga o seu jogador à sua conta: as partidas entram no seu perfil e no ranking. Usuário e senha vazios usam os de fábrica do MiSTer (root / 1).")}</p>
    <p className="muted">
      {t("Prefere fazer à mão?")}{" "}
      <button type="button" className="text-action" disabled={mail.kind === "sending"} onClick={() => void sendByEmail()}>{t(mail.kind === "sending" ? "Enviando…" : "Receber o token por e-mail")}</button>
    </p>
    {mail.text && <p className={mail.kind === "error" ? "mode-error" : "muted"} role="status">{mail.text}</p>}
  </div>;
}
