import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { useDialogFocus } from "./useDialogFocus";

const SOURCES = [
  { id: "steam", title: "Steam", icon: "S", detail: "Biblioteca da Steam", body: "Conecte sua biblioteca para reunir os jogos da Steam na coleção.", availability: "O lançamento pela Steam já existe, mas a sincronização da biblioteca ainda não foi integrada." },
  { id: "fightcade", title: "Fightcade", icon: "F", detail: "Jogos arcade do Fightcade", body: "Escolha os jogos da instalação do Fightcade para organizar sua coleção.", availability: "A leitura da instalação do Fightcade ainda não foi integrada." },
  { id: "emulator", title: "Emuladores", icon: "E", detail: "Bibliotecas de jogos emulados", body: "Reúna os jogos dos seus emuladores na mesma coleção.", availability: "A leitura de bibliotecas de emuladores ainda não foi integrada." },
  { id: "local", title: "Jogos locais", icon: "▣", detail: "Jogos instalados no computador", body: "Escolha os jogos instalados no computador que deseja incluir na coleção.", availability: "A busca por jogos locais ainda não foi integrada." },
] as const;

export default function ImportGamesDialog({ onClose, onViewCollection }: { onClose: () => void; onViewCollection?: () => void }) {
  const { t } = useI18n();
  const [sourceId,setSourceId] = useState<string|null>(null);
  const [showCandidates,setShowCandidates] = useState(false);
  const source = SOURCES.find((candidate)=>candidate.id===sourceId);
  function back() { if(showCandidates)setShowCandidates(false);else if(sourceId)setSourceId(null);else onClose(); }
  const dialogRef = useDialogFocus(true,back);
  useEffect(()=>{dialogRef.current?.querySelector<HTMLButtonElement>("button")?.focus();},[sourceId,showCandidates,dialogRef]);
  return <div className="controller-dialog-backdrop">
    <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby="import-dialog-title" tabIndex={-1} className="controller-dialog import-games-dialog">
      <header><div><h2 id="import-dialog-title">{t(showCandidates ? "Selecionar jogos" : source ? "Conectar {platform}" : "Importar jogos",{platform:source?t(source.title):""})}</h2><p>{t(source ? source.detail : "Escolha de onde deseja trazer seus jogos.")}</p></div><div className="import-header-actions">{source && <button data-dialog-close className="secondary-action" onClick={back}>{t("Voltar")}</button>}<button data-dialog-close={!source ? true : undefined} className="secondary-action" onClick={onClose}>{t("Fechar")}</button></div></header>
      <div className="controller-dialog-body">
        {!source && <><div className="import-source-list">{SOURCES.map((item)=><button key={item.id} className="import-source-option" onClick={()=>setSourceId(item.id)}><span className="platform-mark" aria-hidden="true">{item.icon}</span><span><strong>{t(item.title)}</strong><small>{t(item.detail)}</small></span><span className="import-availability">{t("Conexão indisponível")}</span></button>)}</div><p className="muted">{t("A coleção atual usa dados de demonstração. Nenhuma biblioteca foi conectada ou sincronizada.")}</p></>}
        {source && !showCandidates && <><div className="import-platform-heading"><span className="platform-mark" aria-hidden="true">{source.icon}</span><h3>{t(source.title)}</h3></div><p className="muted">{t(source.body)}</p><ol className="import-process"><li>{t("Ler os jogos disponíveis na origem")}</li><li>{t("Escolher os jogos para a coleção")}</li><li>{t("Conferir o resultado da importação")}</li></ol><div className="integration-notice" role="status"><strong>{t("Conexão indisponível")}</strong><p>{t(source.availability)}</p></div><p className="muted">{t("A conexão com esta biblioteca ainda não está disponível nesta versão.")}</p>{source.id!=="steam" && <button className="secondary-action" onClick={()=>setShowCandidates(true)}>{t("Ver seleção de jogos")}</button>}</>}
        {source && showCandidates && <div className="import-candidates-empty"><span aria-hidden="true">▦</span><h3>{t("Nenhum jogo disponível para selecionar")}</h3><p>{t(source.availability)}</p><p className="muted">{t("A lista será preenchida após a leitura real da biblioteca. Nenhum jogo foi importado.")}</p><button className="primary-action" disabled>{t("Importar selecionados")}</button></div>}
      </div>
      <footer><span className="muted">{t("Catálogo demonstrativo")}</span>{onViewCollection && <button className="secondary-action" onClick={()=>{onClose();onViewCollection();}}>{t("Ver coleção")}</button>}{!onViewCollection && <button className="secondary-action" onClick={onClose}>{t("Voltar à coleção")}</button>}</footer>
    </div>
  </div>;
}
