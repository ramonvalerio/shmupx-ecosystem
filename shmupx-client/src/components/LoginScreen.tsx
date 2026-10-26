import { isTauri } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";

export default function LoginScreen({ busy, error, onSignIn }: { busy: boolean; error?: string; onSignIn: () => void }) {
  const { t } = useI18n();
  return (
    <main className="client-shell login-screen">
      <section className="dashboard-panel login-card" aria-labelledby="login-title">
        <img src="/img/logo.png" alt="ShmupX Client" />
        <h1 id="login-title">{t("Entrar no ShmupX")}</h1>
        <p className="muted">{t("Use sua conta Google. A permissão para publicar no YouTube será pedida separadamente, ao conectar seu canal.")}</p>
        <button type="button" className="primary-action" disabled={busy} onClick={onSignIn}>{busy ? t("Aguardando o Google…") : t("Entrar com Google")}</button>
        {busy && isTauri() && <p className="muted" role="status">{t("Conclua o login no navegador que foi aberto e volte para o aplicativo.")}</p>}
        {error && <p className="mode-error" role="alert">{t(error)}</p>}
      </section>
    </main>
  );
}
