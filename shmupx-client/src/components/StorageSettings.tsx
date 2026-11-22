import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "../i18n";

interface Destino {
  pasta: string;
  padrao: boolean;
  indisponivel: string | null;
  disco_livre: number | null;
  disco_total: number | null;
  gravacoes: number;
  gravacoes_bytes: number;
}

/** Bytes em GB/MB legível (1 casa). */
function size(bytes: number | null, locale: string): string {
  if (bytes === null) return "—";
  const gb = bytes / 1024 ** 3;
  return gb >= 1
    ? `${gb.toLocaleString(locale, { maximumFractionDigits: 1 })} GB`
    : `${(bytes / 1024 ** 2).toLocaleString(locale, { maximumFractionDigits: 1 })} MB`;
}

/**
 * Configurações › Armazenamento: a pasta onde as runs são gravadas (escolher, abrir, voltar ao
 * padrão), com o espaço livre do disco e o quanto os vídeos do ShmupX já ocupam nela.
 */
export default function StorageSettings({ onShowRuns }: { onShowRuns: () => void }) {
  const { t, locale } = useI18n();
  const [destino, setDestino] = useState<Destino | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    if (!isTauri()) return;
    try { setDestino(await invoke<Destino>("gravacoes_destino")); setError(null); }
    catch (failure) { setError(String(failure)); }
  }, []);
  useEffect(() => { void load(); }, [load]);

  async function choose() {
    const pasta = await open({ directory: true, multiple: false, title: t("Pasta das gravações"), defaultPath: destino?.pasta });
    if (typeof pasta !== "string") return;
    await apply(pasta);
  }

  async function apply(pasta: string | null) {
    setBusy(true);
    setError(null);
    try { setDestino(await invoke<Destino>("gravacoes_definir_destino", { pasta })); }
    catch (failure) { setError(String(failure)); }
    finally { setBusy(false); }
  }

  const used = destino && destino.disco_total !== null && destino.disco_livre !== null ? 1 - destino.disco_livre / destino.disco_total : null;
  const lowSpace = destino?.disco_livre !== null && destino?.disco_livre !== undefined && destino.disco_livre < 5 * 1024 ** 3;

  return <div className="storage-settings">
    <div className="storage-folder">
      <code title={destino?.pasta}>{destino?.pasta ?? t("Carregando…")}</code>
      <span className={`play-tag ${destino?.padrao ? "" : "is-owned"}`}>{t(destino?.padrao ? "Padrão" : "Escolhida")}</span>
    </div>
    {destino?.indisponivel && <p className="mode-error-inline" role="alert">{t("A pasta {pasta} não está disponível agora (disco removido?). As runs estão indo para a pasta padrão.", { pasta: destino.indisponivel })}</p>}
    <div className="storage-actions">
      <button type="button" className="primary-action" disabled={busy || !isTauri()} onClick={() => void choose()}>{t("Escolher pasta…")}</button>
      <button type="button" className="secondary-action" disabled={!destino} onClick={() => void invoke("gravacoes_abrir_pasta").catch((failure) => setError(String(failure)))}>{t("Abrir pasta")}</button>
      {destino && !destino.padrao && <button type="button" className="text-action" disabled={busy} onClick={() => void apply(null)}>{t("Voltar à pasta padrão")}</button>}
    </div>

    {destino && <dl className="storage-facts">
      <div><dt>{t("Espaço livre no disco")}</dt><dd className={lowSpace ? "is-low" : ""}>{size(destino.disco_livre, locale)}{destino.disco_total !== null && <small> {t("de {total}", { total: size(destino.disco_total, locale) })}</small>}</dd></div>
      <div><dt>{t("Ocupado pelas gravações do ShmupX")}</dt><dd>{size(destino.gravacoes_bytes, locale)} <small>{t("({count} vídeos)", { count: destino.gravacoes })}</small></dd></div>
    </dl>}
    {used !== null && (
      <div className="storage-bar" role="img" aria-label={t("Disco {percent}% ocupado", { percent: Math.round(used * 100) })}>
        <span style={{ width: `${Math.min(100, used * 100)}%` }} />
        {destino && destino.disco_total ? <span className="storage-bar-runs" style={{ width: `${Math.min(100, (destino.gravacoes_bytes / destino.disco_total) * 100)}%` }} /> : null}
      </div>
    )}
    {lowSpace && <p className="mode-error-inline">{t("Pouco espaço livre: uma run longa pode não caber. Libere espaço ou escolha outra pasta.")}</p>}
    {error && <p className="mode-error-inline" role="alert">{t(error)}</p>}
    <p className="muted">{t("Vale para as próximas runs; os vídeos já gravados continuam onde estão.")}</p>
    <button type="button" className="text-action" onClick={onShowRuns}>{t("Ver runs desta sessão →")}</button>
  </div>;
}
