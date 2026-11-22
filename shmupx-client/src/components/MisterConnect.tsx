import { useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";

/**
 * Conectar o MiSTer FPGA (ADR 008): o token vai para o e-mail da conta, nunca para a tela. Usado em
 * Importar jogos (janela da coleção e Configurações).
 */
export default function MisterConnect() {
  const { t } = useI18n();
  const [state, setState] = useState<{ kind: "idle" | "sending" | "sent" | "error"; text?: string }>({ kind: "idle" });
  const send = async () => {
    if (!isTauri()) { setState({ kind: "error", text: t("Entre com sua conta para conectar o MiSTer.") }); return; }
    setState({ kind: "sending" });
    try {
      const reply = await invoke<{ enviado_para: string }>("mister_enviar_token");
      setState({ kind: "sent", text: t("Token enviado para {email}. Cole a linha do e-mail no shmupx.ini do MiSTer.", { email: reply.enviado_para }) });
    } catch (error) {
      setState({ kind: "error", text: error instanceof Error ? error.message : String(error) });
    }
  };
  return <>
    <button className="primary-action" disabled={state.kind === "sending"} onClick={() => void send()}>{t(state.kind === "sending" ? "Enviando…" : "Enviar token para meu e-mail")}</button>
    {state.text && <p className={state.kind === "error" ? "mode-error" : "muted"} role="status">{state.text}</p>}
    <p className="muted">{t("Cada pedido gera um token novo; os anteriores continuam valendo.")}</p>
  </>;
}
