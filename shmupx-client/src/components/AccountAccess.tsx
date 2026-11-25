import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { looksLikeEmail, MIN_PASSWORD } from "../auth/firebase";
import type { Auth } from "../auth/useAuth";
import { useI18n } from "../i18n";

type Mode = "signin" | "signup" | "reset";

/**
 * Acesso à conta, igual no Client e no Lite: e-mail e senha (com cadastro e "Esqueci minha
 * senha") ou Google. Um cadastro por senha só entra depois de confirmar o e-mail; se o mesmo
 * e-mail entrar depois pelo Google, o Google fica associado ao mesmo cadastro.
 * `primaryClass`/`secondaryClass`: os botões seguem o visual de cada app.
 */
export default function AccountAccess({ auth, primaryClass, secondaryClass }: { auth: Auth; primaryClass: string; secondaryClass: string }) {
  const { state } = auth;
  const [mode, setMode] = useState<Mode>("signin");
  const [email, setEmail] = useState("");
  if (state.status === "verify-email") return <VerifyEmail auth={auth} email={state.email} primaryClass={primaryClass} secondaryClass={secondaryClass} />;
  if (state.status === "signed-out" && state.linkEmail !== undefined) {
    return <LinkGoogle auth={auth} email={state.linkEmail} primaryClass={primaryClass}
      onForgot={() => { setEmail(state.linkEmail ?? ""); setMode("reset"); auth.cancelSignIn(); }} />;
  }
  const googleBusy = state.status === "signed-out" && !!state.busy;
  const googleError = state.status === "signed-out" ? state.error : undefined;
  return (
    <>
      {mode === "signin" && <SignInForm auth={auth} email={email} onEmail={setEmail} primaryClass={primaryClass} onMode={setMode} disabled={googleBusy} />}
      {mode === "signup" && <SignUpForm auth={auth} email={email} onEmail={setEmail} primaryClass={primaryClass} onMode={setMode} />}
      {mode === "reset" && <ResetForm auth={auth} email={email} onEmail={setEmail} primaryClass={primaryClass} onMode={setMode} />}
      {mode !== "reset" && <GoogleButton auth={auth} busy={googleBusy} error={googleError} secondaryClass={secondaryClass} />}
    </>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return <label className="auth-field"><span>{label}</span>{children}</label>;
}

function Message({ error, info }: { error?: string | null; info?: string | null }) {
  const { t } = useI18n();
  if (error) return <p className="auth-error" role="alert">{t(error)}</p>;
  if (info) return <p className="auth-info" role="status">{t(info)}</p>;
  return null;
}

interface FormProps { auth: Auth; email: string; onEmail: (email: string) => void; primaryClass: string; onMode: (mode: Mode) => void }

function SignInForm({ auth, email, onEmail, primaryClass, onMode, disabled }: FormProps & { disabled: boolean }) {
  const { t } = useI18n();
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!looksLikeEmail(email)) { setError("Esse e-mail não parece válido. Confira e tente de novo."); return; }
    setBusy(true); setError(null);
    const problem = await auth.signInEmail(email, password);
    // Deu certo: o onAuthStateChanged troca de tela e este formulário desmonta.
    if (problem) { setError(problem); setBusy(false); }
  }
  return (
    <form className="auth-form" onSubmit={submit} noValidate>
      <Field label={t("E-mail")}><input type="email" required autoComplete="email" value={email} onChange={(event) => onEmail(event.target.value)} /></Field>
      <Field label={t("Senha")}><input type="password" required autoComplete="current-password" value={password} onChange={(event) => setPassword(event.target.value)} /></Field>
      <Message error={error} />
      <button type="submit" className={primaryClass} disabled={busy || disabled || !email || !password}>{busy ? t("Entrando…") : t("Entrar")}</button>
      <div className="auth-links">
        <button type="button" className="text-action" onClick={() => onMode("reset")}>{t("Esqueci minha senha")}</button>
        <button type="button" className="text-action" onClick={() => onMode("signup")}>{t("Criar conta")}</button>
      </div>
    </form>
  );
}

function SignUpForm({ auth, email, onEmail, primaryClass, onMode }: FormProps) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function submit(event: FormEvent) {
    event.preventDefault();
    const problem = !name.trim() ? "Informe um nome de 1 a 32 caracteres."
      : !looksLikeEmail(email) ? "Esse e-mail não parece válido. Confira e tente de novo."
      : password.length < MIN_PASSWORD ? "Escolha uma senha mais forte, com pelo menos 8 caracteres."
      : password !== confirm ? "As duas senhas não são iguais. Digite de novo."
      : null;
    if (problem) { setError(problem); return; }
    setBusy(true); setError(null);
    const failure = await auth.signUpEmail(name, email, password);
    if (failure) { setError(failure); setBusy(false); }
  }
  return (
    <form className="auth-form" onSubmit={submit} noValidate>
      <Field label={t("Nome de piloto")}><input required maxLength={32} autoComplete="nickname" value={name} onChange={(event) => setName(event.target.value)} /></Field>
      <Field label={t("E-mail")}><input type="email" required autoComplete="email" value={email} onChange={(event) => onEmail(event.target.value)} /></Field>
      <Field label={t("Senha (mínimo 8 caracteres)")}><input type="password" required autoComplete="new-password" value={password} onChange={(event) => setPassword(event.target.value)} /></Field>
      <Field label={t("Repita a senha")}><input type="password" required autoComplete="new-password" value={confirm} onChange={(event) => setConfirm(event.target.value)} /></Field>
      <p className="auth-note">{t("Use um e-mail que você abre: vamos mandar um link para confirmar. Se for um e-mail do Google, depois você também pode entrar com o Google, na mesma conta.")}</p>
      <Message error={error} />
      <button type="submit" className={primaryClass} disabled={busy}>{busy ? t("Criando a conta…") : t("Criar conta")}</button>
      <div className="auth-links"><button type="button" className="text-action" onClick={() => onMode("signin")}>{t("Já tenho conta")}</button></div>
    </form>
  );
}

function ResetForm({ auth, email, onEmail, primaryClass, onMode }: FormProps) {
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sent, setSent] = useState(false);
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!looksLikeEmail(email)) { setError("Esse e-mail não parece válido. Confira e tente de novo."); return; }
    setBusy(true); setError(null);
    const problem = await auth.resetPassword(email);
    setBusy(false);
    if (problem) setError(problem); else setSent(true);
  }
  return (
    <form className="auth-form" onSubmit={submit} noValidate>
      <p className="auth-note">{t("Digite o e-mail da sua conta. Vamos mandar um link para você criar uma senha nova.")}</p>
      <Field label={t("E-mail")}><input type="email" required autoComplete="email" value={email} onChange={(event) => { onEmail(event.target.value); setSent(false); }} /></Field>
      <Message error={error} info={sent ? "Pronto. Se esse e-mail tiver conta no ShmupX, o link chega em alguns minutos. Confira também a caixa de spam." : null} />
      <button type="submit" className={primaryClass} disabled={busy || !email}>{busy ? t("Enviando…") : sent ? t("Enviar de novo") : t("Enviar link")}</button>
      <div className="auth-links"><button type="button" className="text-action" onClick={() => onMode("signin")}>{t("← Voltar para entrar")}</button></div>
    </form>
  );
}

function GoogleButton({ auth, busy, error, secondaryClass }: { auth: Auth; busy: boolean; error?: string; secondaryClass: string }) {
  const { t } = useI18n();
  return (
    <div className="auth-google">
      <div className="auth-divider"><span>{t("ou")}</span></div>
      <button type="button" className={secondaryClass} disabled={busy} onClick={() => void auth.signIn()}>{busy ? t("Aguardando o Google…") : t("Entrar com Google")}</button>
      {busy && isTauri() && <p className="auth-note" role="status">
        {t("Escolha sua conta Google no navegador que abriu. Ao terminar, o ShmupX volta sozinho para a frente.")}
        {" "}<button type="button" className="text-action" onClick={auth.cancelSignIn}>{t("Cancelar")}</button>
      </p>}
      <Message error={error} />
    </div>
  );
}

function LinkGoogle({ auth, email, primaryClass, onForgot }: { auth: Auth; email: string; primaryClass: string; onForgot: () => void }) {
  const { t } = useI18n();
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true); setError(null);
    const problem = await auth.linkGoogle(password);
    if (problem) { setError(problem); setBusy(false); }
  }
  return (
    <form className="auth-form" onSubmit={submit} noValidate>
      <h2 className="auth-title">{t("Esse e-mail já tem conta")}</h2>
      <p className="auth-note">{t("{email} já tem uma conta ShmupX criada com senha. Digite a senha dela uma vez para ligar o Google à mesma conta. Depois disso, é só entrar com o Google.", { email })}</p>
      <Field label={t("Senha")}><input type="password" required autoFocus autoComplete="current-password" value={password} onChange={(event) => setPassword(event.target.value)} /></Field>
      <Message error={error} />
      <button type="submit" className={primaryClass} disabled={busy || !password}>{busy ? t("Ligando…") : t("Ligar o Google e entrar")}</button>
      <div className="auth-links">
        <button type="button" className="text-action" onClick={onForgot}>{t("Esqueci minha senha")}</button>
        <button type="button" className="text-action" onClick={auth.cancelSignIn}>{t("Cancelar")}</button>
      </div>
    </form>
  );
}

const RESEND_WAIT = 60;

function VerifyEmail({ auth, email, primaryClass, secondaryClass }: { auth: Auth; email: string; primaryClass: string; secondaryClass: string }) {
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [wait, setWait] = useState(RESEND_WAIT);
  const { checkVerified } = auth;

  // O jogador confirma no navegador: confere sozinho de tempos em tempos e ao voltar para o app.
  useEffect(() => {
    const check = () => { void checkVerified(); };
    const timer = window.setInterval(check, 5000);
    window.addEventListener("focus", check);
    return () => { window.clearInterval(timer); window.removeEventListener("focus", check); };
  }, [checkVerified]);
  useEffect(() => {
    if (wait <= 0) return;
    const timer = window.setTimeout(() => setWait((left) => left - 1), 1000);
    return () => window.clearTimeout(timer);
  }, [wait]);

  async function confirmed() {
    setBusy(true); setError(null); setInfo(null);
    if (!(await checkVerified())) { setError("Ainda não recebemos a confirmação. Abra o e-mail do ShmupX, clique no link e tente de novo."); setBusy(false); }
  }
  async function resend() {
    setError(null); setInfo(null);
    const problem = await auth.resendVerification();
    if (problem) setError(problem); else { setInfo("E-mail reenviado. Confira também a caixa de spam."); setWait(RESEND_WAIT); }
  }
  return (
    <div className="auth-form">
      <h2 className="auth-title">{t("Confirme seu e-mail")}</h2>
      <p className="auth-note">{t("Enviamos um link de confirmação para {email}. Abra o e-mail, clique no link e volte aqui: o ShmupX continua sozinho.", { email })}</p>
      <Message error={error} info={info} />
      <button type="button" className={primaryClass} disabled={busy} onClick={() => void confirmed()}>{busy ? t("Conferindo…") : t("Já confirmei")}</button>
      <button type="button" className={secondaryClass} disabled={wait > 0} onClick={() => void resend()}>{wait > 0 ? t("Reenviar e-mail ({seconds}s)", { seconds: wait }) : t("Reenviar e-mail")}</button>
      <div className="auth-links"><button type="button" className="text-action" onClick={() => void auth.signOut()}>{t("Usar outra conta")}</button></div>
    </div>
  );
}
