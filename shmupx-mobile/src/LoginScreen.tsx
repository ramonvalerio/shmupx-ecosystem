import { useI18n } from "@client/i18n";

export default function LoginScreen({ busy, error, onSignIn }: { busy: boolean; error?: string; onSignIn: () => void }) {
  const { t } = useI18n();
  return (
    <main className="lite-screen lite-login">
      <img src="/img/logo.png" alt="ShmupX" className="lite-logo" />
      <p className="lite-badge">LITE</p>
      <p className="lite-muted">{t("Acompanhe as salas, converse no chat e assista às lives pelo celular.")}</p>
      <button type="button" className="lite-primary" onClick={onSignIn} disabled={busy}>
        {busy ? t("Conclua o login no navegador e volte ao app…") : t("Entrar com Google")}
      </button>
      {error && <p className="lite-error" role="alert">{t(error)}</p>}
      <p className="lite-note">{t("Para jogar e gravar runs, use o ShmupX Client no computador.")}</p>
    </main>
  );
}
