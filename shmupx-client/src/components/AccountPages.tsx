import { useI18n } from "../i18n";
import { useState, type FormEvent } from "react";
import { PageHeading } from "./ClientPages";
import type { GameRoom } from "../types/room";
import type { SessionRun } from "../types/submission";
import { updateDisplayName, type PlayerProfile } from "../auth/firebase";

const RUN_LABELS: Record<SessionRun["status"], string> = {
  recording: "Gravando",
  analyzing: "Analisando",
  failed: "Não enviada",
  submitted: "Enviada",
  expired: "Tempo encerrado",
};

function runDate(timestamp: number, locale: string) {
  return new Date(timestamp).toLocaleString(locale, { dateStyle: "short", timeStyle: "short" });
}

export function SubmissionsPage({ runs, rooms, onOpenGame }: {
  runs: SessionRun[];
  rooms: GameRoom[];
  onOpenGame: (id: string) => void;
}) {
  const { t, locale } = useI18n();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const gameTitle = (id: string) => rooms.find((room) => room.id === id)?.title ?? id;
  const visible = runs.filter((run) => gameTitle(run.gameId).toLocaleLowerCase("pt-BR").includes(query.toLocaleLowerCase("pt-BR")) && (filter === "all" || run.status === filter));
  const selected = visible.find((run) => run.id === selectedId);

  return (
    <div className="client-page">
      <PageHeading title={t("Submissões")} detail={t("Acompanhe as gravações e os envios observados nesta sessão.")} />
      <div className="page-filters">
        <label>{t("Buscar jogo")}<input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("Nome do jogo…")} /></label>
        <label>{t("Status")}<select value={filter} onChange={(event) => setFilter(event.target.value)}><option value="all">{t("Todos")}</option>{Object.entries(RUN_LABELS).map(([value, label]) => <option key={value} value={value}>{t(label)}</option>)}</select></label>
        <span className="muted">{visible.length} {t(" de ")}{runs.length} {t(" runs")}</span>
      </div>
      <div className="master-detail">
        <div className="submission-list">
          {visible.map((run) => (
            <button className={`submission-row ${selectedId === run.id ? "selected-row" : ""}`} key={run.id} onClick={() => setSelectedId(run.id)} aria-pressed={selectedId === run.id}>
              <span className="game-initial">{gameTitle(run.gameId).slice(0, 2).toUpperCase()}</span>
              <span><strong>{gameTitle(run.gameId)}</strong><small>{runDate(run.startedAt, locale)}</small></span>
              <span className={`run-badge run-${run.status}`}>{t(RUN_LABELS[run.status])}</span>
            </button>
          ))}
          {visible.length === 0 && <div className="empty-state"><h2>{runs.length ? t("Nenhuma run corresponde aos filtros") : t("Sua primeira run aparece aqui")}</h2><p>{runs.length ? t("Altere a busca ou o status para consultar outras runs.") : t("Escolha um jogo na coleção. A run será listada quando o início da gravação for confirmado.")}</p>{!runs.length && rooms[0] && <button className="primary-action" onClick={() => onOpenGame(rooms[0].id)}>{t("Abrir coleção")}</button>}</div>}
        </div>
        <article className="dashboard-panel">
          <h2>{selected ? gameTitle(selected.gameId) : t("Detalhes da run")}</h2>
          {selected ? <><dl className="detail-list"><dt>{t("Início da run")}</dt><dd>{runDate(selected.startedAt, locale)}</dd><dt>{t("Fim")}</dt><dd>{selected.endedAt ? runDate(selected.endedAt, locale) : t("Em andamento")}</dd><dt>{t("Status")}</dt><dd>{t(RUN_LABELS[selected.status])}</dd><dt>{t("Placar e verificação")}</dt><dd>{t("Não disponibilizados pelo serviço para esta tela.")}</dd></dl><p className="muted">{selected.status === "submitted" ? t("O comando de encerramento e envio terminou com sucesso. A posição no ranking ainda não foi consultada.") : selected.status === "expired" ? t("O limite da ficha encerrou a gravação. A confirmação do envio ainda não foi recebida por esta tela.") : selected.status === "analyzing" ? t("A gravação terminou. A gameplay está sendo analisada localmente.") : selected.status === "failed" ? t("A run não foi enviada. Consulte o motivo abaixo.") : t("A gravação está em andamento. Abra a sala para acessar os controles da run.")}</p>{selected.message && <p className="muted" role="status">{selected.message}</p>}<button className="secondary-action" onClick={() => onOpenGame(selected.gameId)}>{t("Abrir sala do jogo")}</button></> : <p className="muted">{t("Selecione uma run para consultar seu estado.")}</p>}
        </article>
      </div>
      <p className="muted">{t("Histórico desta sessão, sem sincronização com runs anteriores. Nenhum placar é gerado por esta tela.")}</p>
    </div>
  );
}

export function ProfilePage({ profile, onProfile, onSubscription }: { profile: PlayerProfile; onProfile: (profile: PlayerProfile) => void; onSubscription: () => void }) {
  const { t } = useI18n();
  const [name, setName] = useState(profile.displayName);
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);
  async function saveProfile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const displayName = name.trim();
    if (!displayName || displayName.length > 32) { setMessage("Informe um nome de 1 a 32 caracteres."); return; }
    setSaving(true);
    try { await updateDisplayName(profile.uid, displayName); onProfile({ ...profile, displayName }); setMessage("Perfil salvo na sua conta."); }
    catch { setMessage("Não foi possível salvar o perfil. Verifique a conexão e tente novamente."); }
    finally { setSaving(false); }
  }
  return <div className="client-page"><PageHeading title={t("Meu perfil")} detail={t("Sua conta ShmupX, conectada pelo Google.")} /><div className="profile-layout"><article className="dashboard-panel profile-summary"><span className="profile-avatar" aria-hidden="true">{profile.displayName.slice(0, 2).toUpperCase()}</span><h2>{profile.displayName}</h2><span className="sample-chip">{t("CONTA GOOGLE")}</span><p className="muted">{t("Fichas, gameplays e rankings ainda não estão ligados a esta conta.")}</p></article><article className="dashboard-panel settings-content"><h2>{t("Informações do jogador")}</h2><form onSubmit={saveProfile}><label className="settings-field">{t("Nome de exibição")}<input required maxLength={32} value={name} onChange={(event) => setName(event.target.value)} autoComplete="nickname" /></label><button className="primary-action" type="submit" disabled={saving}>{t("Salvar perfil")}</button><p className="muted" role="status">{t(message)}</p></form><hr /><h3>{t("Assinatura")}</h3><p className="muted">{t("Consulte a prévia de assinatura do cliente.")}</p><button className="secondary-action" onClick={onSubscription}>{t("Ver assinatura")}</button></article></div></div>;
}

export function SubscriptionPage({ onEvents }: { onEvents: () => void }) {
  const { t } = useI18n();
  const [method, setMethod] = useState<"card" | "pix">("card");
  const [accepted, setAccepted] = useState(false);
  const [reviewed, setReviewed] = useState(false);
  return <div className="client-page"><button className="text-action" onClick={onEvents}>{t("← Voltar aos eventos")}</button><PageHeading title={t("Um novo nível para sua experiência.")} detail={t("Revise a prévia da assinatura e conheça as opções propostas.")} /><p className="muted">{t("Assinatura → Forma de pagamento → Revisão")}</p><div className="subscription-layout"><article className="dashboard-panel"><h2>{t("Forma de pagamento")}</h2><div className="payment-choices" role="group" aria-label={t("Forma de pagamento demonstrativa")}><button aria-pressed={method === "card"} onClick={() => { setMethod("card"); setReviewed(false); }}>{t("Cartão")}</button><button aria-pressed={method === "pix"} onClick={() => { setMethod("pix"); setReviewed(false); }}>{t("Pix")}</button></div>{method === "card" ? <div className="payment-example"><label>{t("Número do cartão")}<input readOnly value="•••• •••• •••• 4242" /></label><label>{t("Nome no cartão")}<input readOnly value={t("JOGADOR DEMONSTRAÇÃO")} /></label><div className="payment-field-row"><label>{t("Validade")}<input readOnly value={t("MM/AA")} /></label><label>{t("Código de segurança")}<input readOnly value="•••" /></label></div></div> : <div className="pix-preview"><h3>{t("Pagamento com Pix")}</h3><p className="muted">{t("O código e o QR Code serão disponibilizados quando o pagamento real for habilitado.")}</p><span className="sample-chip">{t("PRÉVIA · SEM CÓDIGO DE PAGAMENTO")}</span></div>}<label className="checkbox-field"><input type="checkbox" checked={accepted} onChange={(event) => { setAccepted(event.target.checked); setReviewed(false); }} />{t("Li o resumo da assinatura demonstrativa.")}</label><p className="muted">{t("Campos de exemplo, somente para leitura.")}</p></article><article className="dashboard-panel"><p className="eyebrow">{t("RESUMO DA ASSINATURA")}</p><h2>{t("ShmupX Client")}</h2><p className="muted">{t("Plano e benefícios a definir.")}</p><dl className="detail-list"><dt>{t("Frequência")}</dt><dd>{t("A definir")}</dd><dt>{t("Renovação")}</dt><dd>{t("A definir")}</dd><dt>{t("Total")}</dt><dd className="score-value">{t("Valor a definir")}</dd></dl><button className="primary-action" disabled={!accepted} onClick={() => setReviewed(true)}>{t("Revisar assinatura")}</button><p className="muted">{t("Prévia do checkout. Nenhuma cobrança será realizada.")}</p>{reviewed && <p className="review-feedback" role="status">{t("Resumo revisado. Plano, valor e condições precisam ser definidos para habilitar o pagamento real.")}</p>}</article></div></div>;
}
