import { isTauri } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import type { Auth } from "../auth/useAuth";
import AccountAccess from "./AccountAccess";

// Layout do design "Acesso do Piloto" (Superdesign). Entra com e-mail e senha ou com o Google.
export default function LoginScreen({ auth }: { auth: Auth }) {
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
            <AccountAccess auth={auth} primaryClass="entrance-cta" secondaryClass="entrance-secondary" />
            <div className="entrance-card-foot">{t("CADA RUN CONTA. QUEBRE SEU RECORDE.")}</div>
          </div>
        </section>
      </div>
    </main>
  );
}
