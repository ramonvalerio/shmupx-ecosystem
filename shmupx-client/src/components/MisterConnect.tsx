import { useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../i18n";
import { readMisterLibrary } from "../data/misterLibrary";

type Step = "idle" | "loading" | "choose" | "saving" | "done" | "error";

/**
 * Liga o MiSTer FPGA à conta (ADR 008). Direto pela rede: o Client lê os jogadores do
 * `shmupx.ini`, pede um token novo e grava no MiSTer (a linha do jogador, a lista e o endereço do
 * ShmupX); o token não aparece na tela. Alternativa: receber o token por e-mail e colar à mão.
 * `host`/`password`: os da tela que já leu o MiSTer (Importar jogos); sem eles, pede aqui.
 */
export default function MisterConnect({ host: hostProp, password: passwordProp }: { host?: string; password?: string }) {
  const { t } = useI18n();
  const [host, setHost] = useState(() => hostProp ?? readMisterLibrary()?.host ?? "");
  const [password, setPassword] = useState(passwordProp ?? "");
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
    try {
      const list = await invoke<string[]>("mister_usuarios", { host: target, senha: secret || null });
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
      await invoke<string>("mister_conectar", { host: target, senha: secret || null, username: chosen });
      setStep("done");
      setMessage(t("Pronto: o ShmupX está instalado no MiSTer e {player} está ligado à sua conta. No MiSTer, abra a pasta ShmupX no menu principal; as partidas desse jogador entram no seu perfil e no ranking.", { player: chosen.toUpperCase() }));
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
      <label className="settings-field">{t("Senha do root (vazio = padrão)")}<input type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="off" /></label>
    </div>}
    {(step === "idle" || step === "loading" || step === "error") && (
      <button className="primary-action" disabled={step === "loading" || !target.trim()} onClick={() => void loadPlayers()}>
        {t(step === "loading" ? "Lendo o MiSTer…" : "Conectar ao meu MiSTer")}
      </button>
    )}
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
      <button className="primary-action" disabled={step === "saving" || !chosen} onClick={() => void connect()}>
        {t(step === "saving" ? "Instalando no MiSTer…" : "Instalar e ligar à minha conta")}
      </button>
      {progress && <p className="muted" role="status">{t(progress)}</p>}
    </>}
    {message && <p className={step === "error" ? "mode-error" : "success-text"} role="status">{t(message)}</p>}
    <p className="muted">{t("Pelo SSH do MiSTer (usuário root), o Client instala ou atualiza o ShmupX: o agente, o menu ShmupX por jogador e modo, as telas em Scripts, o serviço no boot e, se faltar, o core MegaDrive do Earthion. Depois grava o seu jogador, o endereço do ShmupX e um token novo no shmupx.ini (o anterior fica em shmupx.ini.bak). O token não aparece na tela.")}</p>
    <p className="muted">
      {t("Prefere fazer à mão?")}{" "}
      <button type="button" className="text-action" disabled={mail.kind === "sending"} onClick={() => void sendByEmail()}>{t(mail.kind === "sending" ? "Enviando…" : "Receber o token por e-mail")}</button>
    </p>
    {mail.text && <p className={mail.kind === "error" ? "mode-error" : "muted"} role="status">{mail.text}</p>}
  </div>;
}
