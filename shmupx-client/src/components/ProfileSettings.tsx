import { useEffect, useState, type FormEvent } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { updateDisplayName, type PlayerProfile } from "../auth/firebase";
import { DEFAULT_AVATAR, useMyPublicProfile, type MyPublicProfile } from "../data/publicProfile";
import AvatarEditor from "./AvatarEditor";

const NICK = /^[a-z0-9_]{3,20}$/;
type Check = "idle" | "checking" | "free" | "taken" | "invalid" | "same";

/** Verifica o nick enquanto o jogador digita (espera ele parar um instante). */
function useNickCheck(value: string, current: string | undefined) {
  const [check, setCheck] = useState<Check>("idle");
  useEffect(() => {
    const nick = value.trim().toLowerCase();
    if (!nick) { setCheck("idle"); return; }
    if (nick === current) { setCheck("same"); return; }
    if (!NICK.test(nick)) { setCheck("invalid"); return; }
    setCheck("checking");
    let alive = true;
    const timer = window.setTimeout(() => {
      invoke<{ valid: boolean; available: boolean }>("perfil_nick_disponivel", { nick })
        .then((r) => { if (alive) setCheck(!r.valid ? "invalid" : r.available ? "free" : "taken"); })
        .catch(() => { if (alive) setCheck("idle"); });
    }, 400);
    return () => { alive = false; window.clearTimeout(timer); };
  }, [value, current]);
  return check;
}

function NickStatus({ check }: { check: Check }) {
  const { t } = useI18n();
  const text: Record<Check, string> = {
    idle: "", checking: "Verificando…", free: "✓ Disponível", taken: "✗ Já está em uso", invalid: "De 3 a 20 caracteres: letras minúsculas, números e _", same: "Esse é o seu nick atual",
  };
  if (check === "idle") return null;
  return <small className={`nick-status is-${check}`} role="status">{t(text[check])}</small>;
}

/**
 * Configurações › Perfil: avatar (editor), nome de exibição e nick (@). O nick vem padrão, pode
 * ser trocado uma vez (com verificação de disponibilidade) e depois só por pedido ao suporte, que
 * fica aguardando análise. A assinatura abre a página do site.
 */
export default function ProfileSettings({ profile, onProfile }: { profile: PlayerProfile; onProfile: (profile: PlayerProfile) => void }) {
  const { t } = useI18n();
  const mine = useMyPublicProfile(profile.displayName);
  const me = mine.profile;
  const [editingAvatar, setEditingAvatar] = useState(false);
  const [name, setName] = useState(profile.displayName);
  const [nameMessage, setNameMessage] = useState("");
  const [nick, setNick] = useState("");
  const [nickMessage, setNickMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const check = useNickCheck(nick, me?.handle);
  const pending = me?.handle_request?.status === "pending";

  useEffect(() => { if (me && !nick) setNick(me.handle); }, [me]); // eslint-disable-line react-hooks/exhaustive-deps

  async function saveName(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const displayName = name.trim();
    if (!displayName || displayName.length > 32) { setNameMessage("Informe um nome de 1 a 32 caracteres."); return; }
    try { await updateDisplayName(profile.uid, displayName); onProfile({ ...profile, displayName }); setNameMessage("Nome salvo."); }
    catch { setNameMessage("Não foi possível salvar o nome. Verifique a conexão e tente novamente."); }
  }

  async function saveNick() {
    const value = nick.trim().toLowerCase();
    if (check !== "free") return;
    if (!window.confirm(t("O nick só pode ser trocado uma vez. Depois, só com pedido ao suporte. Confirmar @{nick}?", { nick: value }))) return;
    setBusy(true);
    setNickMessage(null);
    try { mine.apply(await invoke<MyPublicProfile>("perfil_trocar_nick", { nick: value })); setNickMessage("Nick salvo."); }
    catch (failure) { setNickMessage(failure instanceof Error ? failure.message : String(failure)); }
    finally { setBusy(false); }
  }

  async function requestNick() {
    const value = nick.trim().toLowerCase();
    if (check !== "free" || pending) return;
    setBusy(true);
    setNickMessage(null);
    try { mine.apply(await invoke<MyPublicProfile>("perfil_pedir_nick", { nick: value })); setNickMessage("Pedido enviado ao suporte."); }
    catch (failure) { setNickMessage(failure instanceof Error ? failure.message : String(failure)); }
    finally { setBusy(false); }
  }

  return <div className="profile-settings">
    {editingAvatar && <AvatarEditor current={me?.avatar ?? null} onClose={() => setEditingAvatar(false)} onSaved={() => { setEditingAvatar(false); void mine.refresh(); }} />}
    <section className="profile-settings-head">
      <button type="button" className="profile-avatar-button" onClick={() => setEditingAvatar(true)} title={t("Trocar avatar")}>
        <img src={me?.avatar || DEFAULT_AVATAR} alt="" />
        <span>{t("Trocar")}</span>
      </button>
      <div>
        <strong>{profile.displayName}</strong>
        <small className="muted">{me ? `@${me.handle}` : t("Carregando…")}</small>
        <span className="sample-chip">{t("CONTA GOOGLE")}</span>
      </div>
    </section>
    {mine.error && <p className="mode-error-inline" role="alert">{t(mine.error)}</p>}

    <form className="profile-settings-row" onSubmit={saveName}>
      <label className="settings-field">{t("Nome de exibição")}
        <input required maxLength={32} value={name} onChange={(event) => setName(event.target.value)} autoComplete="nickname" />
      </label>
      <button className="secondary-action" type="submit" disabled={name.trim() === profile.displayName}>{t("Salvar nome")}</button>
    </form>
    {nameMessage && <p className="muted" role="status">{t(nameMessage)}</p>}

    <div className="profile-settings-row">
      <label className="settings-field">{t("Nick (ID)")}
        <span className="nick-input">
          <span aria-hidden="true">@</span>
          <input value={nick} maxLength={20} disabled={!me || pending} onChange={(event) => setNick(event.target.value.toLowerCase().replace(/[^a-z0-9_]/g, ""))} autoComplete="off" spellCheck={false} />
        </span>
      </label>
      {me && !me.handle_locked && <button type="button" className="primary-action" disabled={busy || check !== "free"} onClick={() => void saveNick()}>{t("Salvar nick")}</button>}
      {me && me.handle_locked && <button type="button" className="secondary-action" disabled={busy || pending || check !== "free"} onClick={() => void requestNick()}>{t(pending ? "Aguardando análise" : "Pedir troca ao suporte")}</button>}
    </div>
    {!pending && <NickStatus check={check} />}
    <p className="muted">
      {!me ? "" : pending
        ? t("Pedido de troca para @{nick} aguardando análise do suporte.", { nick: me.handle_request?.requested ?? "" })
        : me.handle_locked
          ? t("Seu nick já foi trocado uma vez. Para outra troca, digite o nick desejado e peça ao suporte.")
          : t("Você pode trocar o nick uma vez. Depois de salvo, só com pedido ao suporte.")}
    </p>
    {nickMessage && <p className="muted" role="status">{t(nickMessage)}</p>}

    <section className="profile-settings-subscription">
      <div>
        <strong>{t("Assinatura")}</strong>
        <small className="muted">{t("Acesso Beta · Grátis")}</small>
      </div>
      <button type="button" className="secondary-action" onClick={() => { if (isTauri()) void invoke("site_abrir", { caminho: "/subscription" }); }}>{t("Ver assinatura")}</button>
    </section>
  </div>;
}
