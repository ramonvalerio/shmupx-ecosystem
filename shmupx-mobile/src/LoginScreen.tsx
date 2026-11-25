import { useI18n } from "@client/i18n";
import type { Auth } from "@client/auth/useAuth";
import AccountAccess from "@client/components/AccountAccess";
import { APP_VERSION } from "./version";

export default function LoginScreen({ auth }: { auth: Auth }) {
  const { t } = useI18n();
  return (
    <main className="lite-screen lite-login">
      <img src="/img/logo.png" alt="ShmupX" className="lite-logo" />
      <p className="lite-badge">LITE <span className="lite-version">v{APP_VERSION}</span></p>
      <p className="lite-muted">{t("Acompanhe as salas, converse no chat e assista às lives pelo celular.")}</p>
      <div className="lite-auth"><AccountAccess auth={auth} primaryClass="lite-primary" secondaryClass="lite-secondary" /></div>
      <p className="lite-note">{t("Para jogar e gravar runs, use o ShmupX Client no computador.")}</p>
    </main>
  );
}
