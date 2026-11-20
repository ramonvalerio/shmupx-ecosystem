import { isTauri } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";
import { useI18n } from "../i18n";

// Layout do design "Acesso do Piloto" (Superdesign). O login real é só Google: sem campos de nome/senha.
export default function LoginScreen({ busy, error, onSignIn }: { busy: boolean; error?: string; onSignIn: () => void }) {
  const { t } = useI18n();
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => { if (isTauri()) getVersion().then(setVersion).catch(() => {}); }, []);
  return (
    <main className="client-shell login-screen">
      <div className="entrance-layout">
        <div className="entrance-brand">
          <div>
            <img src="/img/login-logo.png" alt="ShmupX" className="entrance-logo" />
            <div className="entrance-hero">
              <div className="entrance-kicker"><i />{t("O PRÓXIMO NÍVEL É SEU")}</div>
              <h2>{t("Entre no cockpit e faça parte do")}<br /><strong>{t("maior ranking de shmup online.")}</strong></h2>
              <p>{t("Supere seus limites.")} <strong>{t("Quebre seu próprio recorde.")}</strong></p>
            </div>
          </div>
          <span className="entrance-brand-footer">SHMUPX CLIENT{version ? ` v${version}` : ""}</span>
        </div>
        <section className="entrance-panel" aria-labelledby="login-title">
          <div className="entrance-card">
            <i className="entrance-accent-top" /><i className="entrance-accent-bottom" />
            <div className="entrance-card-head"><span><i />{t("ACESSO DO PILOTO")}</span><span className="muted">// 01</span></div>
            <h1 id="login-title">{t("Bem-vindo, piloto.")}</h1>
            <p className="entrance-sub">{t("Sua próxima jornada começa aqui.")}</p>
            <p className="entrance-note">{t("Use sua conta Google. A permissão para publicar no YouTube será pedida separadamente, ao conectar seu canal.")}</p>
            <button type="button" className="entrance-cta" disabled={busy} onClick={onSignIn}>{busy ? t("Aguardando o Google…") : t("Entrar com Google")}<span aria-hidden="true">→</span></button>
            {busy && isTauri() && <p className="entrance-note" role="status">{t("Conclua o login no navegador que foi aberto e volte para o aplicativo.")}</p>}
            {error && <p className="mode-error" role="alert">{t(error)}</p>}
            <div className="entrance-card-foot">{t("CADA RUN CONTA. QUEBRE SEU RECORDE.")}</div>
          </div>
        </section>
      </div>
    </main>
  );
}
