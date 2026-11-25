import { useI18n } from "../i18n";

/**
 * Conta banida ou com o acesso desativado no Users Manager do Lab: no lugar do Client, um aviso
 * simples com o que fazer. Some sozinho se a moderação for desfeita (o documento é lido em tempo real).
 */
export default function AccountBlockedScreen({ banned, email, onSignOut }: { banned: boolean; email?: string | null; onSignOut: () => void }) {
  const { t } = useI18n();
  return (
    <main className="client-shell login-screen">
      <div className="entrance-layout">
        <div className="entrance-brand">
          <div>
            <img src="/img/login-logo.png" alt="ShmupX" className="entrance-logo" />
          </div>
        </div>
        <section className="entrance-panel" aria-labelledby="blocked-title">
          <div className="entrance-card">
            <i className="entrance-accent-top" /><i className="entrance-accent-bottom" />
            <div className="entrance-card-head"><span><i />{t(banned ? "CONTA BANIDA" : "ACESSO DESATIVADO")}</span></div>
            <h1 id="blocked-title">{t(banned ? "Sua conta foi banida do ShmupX." : "O acesso da sua conta está desativado.")}</h1>
            <p className="entrance-sub">
              {t(banned
                ? "Você não pode mais jogar valendo ranking nem participar das salas."
                : "Enquanto estiver desativada, você não joga valendo ranking nem participa das salas.")}
            </p>
            <p className="entrance-note">
              {t("Se acha que é um engano, escreva para shmupx-support@googlegroups.com informando o e-mail da sua conta{email}.", { email: email ? ` (${email})` : "" })}
            </p>
            <button type="button" className="entrance-cta" onClick={onSignOut}>{t("Sair e entrar com outra conta")}<span aria-hidden="true">→</span></button>
          </div>
        </section>
      </div>
    </main>
  );
}
