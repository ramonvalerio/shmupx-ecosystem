import { useI18n } from "../i18n";
import { useEffect, useState, type ReactNode } from "react";
import type { GameRoom } from "../types/room";
import { EventsPage, HelpPage, HomePage, RankingPage, type Section } from "./ClientPages";
import { ProfilePage, SubmissionsPage, SubscriptionPage } from "./AccountPages";
import { useBigPicture } from "./useBigPicture";
import type { SessionRun } from "../types/submission";
import CollectionPane from "./CollectionPane";
import RoomRankingView from "./RoomRankingView";
import RoomTabs from "./RoomTabs";
import SettingsPage from "./SettingsPage";
import { readPreferences, PREFERENCES_KEY } from "./settingsState";
import NotificationBell from "./NotificationBell";
import NotificationPage from "./NotificationPage";
import { useNotifications } from "../data/useNotifications";
import { useAutoUpdate } from "../data/useAutoUpdate";
import { useCountdown, useLive } from "../data/useLive";
import { AMBIENTE, type PlayerProfile } from "../auth/firebase";

// `hidden`: fora do menu por enquanto; a tela continua existindo e abre por outros caminhos.
const NAVIGATION: { id: Section; icon: string; label: string; hidden?: boolean }[] = [
  { id: "home", icon: "⌂", label: "Início" },
  { id: "collection", icon: "▦", label: "Coleção" },
  { id: "events", icon: "◇", label: "Eventos", hidden: true },
  { id: "ranking", icon: "☆", label: "Ranking" },
  { id: "submissions", icon: "↑", label: "Submissões", hidden: true },
  { id: "settings", icon: "⚙", label: "Configurações" },
  { id: "help", icon: "?", label: "Ajuda", hidden: true },
];
interface ClientShellProps {
  rooms: GameRoom[];
  selectedRoomId: string | null;
  recordingRoomId: string | null;
  onSelectRoom: (id: string) => void;
  /** Conteúdo da sala de um jogo; `openRanking` troca a aba para o ranking do jogo. */
  renderRoom: (room: GameRoom, actions: { openRanking: () => void }) => ReactNode;
  runs: SessionRun[];
  profile: PlayerProfile;
  onProfile: (profile: PlayerProfile) => void;
  onSignOut: () => void;
}

export default function ClientShell({ rooms, selectedRoomId, recordingRoomId, onSelectRoom, renderRoom, runs, profile, onProfile, onSignOut }: ClientShellProps) {
  const { t, setLanguage } = useI18n();
  const [preferences, setPreferences] = useState(readPreferences);
  const [section, setSection] = useState<Section>(() => preferences.startup);
  const [storageError, setStorageError] = useState(false);
  const [profileMenuOpen, setProfileMenuOpen] = useState(false);
  const bigPicture = useBigPicture(preferences.startupBigPicture);
  const recordingRoom = rooms.find((room) => room.id === recordingRoomId);
  const notifications = useNotifications();
  const live = useLive();
  const liveCountdown = useCountdown(live.state.status === "live" ? live.state.ends_at_ms : null);
  const liveRoom = rooms.find((room) => room.id === live.state.game_id);
  // Instalar fecha o app: nunca durante uma gravação.
  const autoUpdate = useAutoUpdate(Boolean(recordingRoom));
  const autoUpdateActive = autoUpdate.state.status !== "idle" && autoUpdate.state.status !== "error";
  const [notificationId, setNotificationId] = useState<string | null>(null);
  function openNotification(id: string | null) { setNotificationId(id); setSection("notification"); }

  useEffect(() => {
    try { localStorage.setItem(PREFERENCES_KEY, JSON.stringify(preferences)); setStorageError(false); }
    catch { setStorageError(true); }
  }, [preferences]);

  // Salas abertas em abas; `activeRoomId` null mostra a grade da coleção.
  const [openRoomIds, setOpenRoomIds] = useState<string[]>([]);
  const [activeRoomId, setActiveRoomId] = useState<string | null>(null);
  const [rankingRoomIds, setRankingRoomIds] = useState<string[]>([]);
  const activeRoom = rooms.find((room) => room.id === activeRoomId) ?? null;

  // Um jogo despublicado sai do catálogo: a aba dele fecha.
  useEffect(() => {
    const valid = (id: string) => rooms.some((room) => room.id === id);
    setOpenRoomIds((current) => (current.every(valid) ? current : current.filter(valid)));
    setActiveRoomId((current) => (current === null || valid(current) ? current : null));
  }, [rooms]);

  function openGame(id: string) {
    setOpenRoomIds((current) => (current.includes(id) ? current : [...current, id]));
    setActiveRoomId(id);
    onSelectRoom(id);
    setSection("collection");
  }
  function activateRoom(id: string) { setActiveRoomId(id); onSelectRoom(id); }
  function closeRoom(id: string) {
    const index = openRoomIds.indexOf(id);
    const remaining = openRoomIds.filter((roomId) => roomId !== id);
    setOpenRoomIds(remaining);
    setRankingRoomIds((current) => current.filter((roomId) => roomId !== id));
    if (activeRoomId === id) {
      const next = remaining[Math.min(index, remaining.length - 1)] ?? null;
      setActiveRoomId(next);
      if (next) onSelectRoom(next);
    }
  }
  function setRanking(id: string, open: boolean) {
    setRankingRoomIds((current) => (open ? [...current.filter((roomId) => roomId !== id), id] : current.filter((roomId) => roomId !== id)));
  }
  const isLibrary = section === "collection";

  return (
    <main ref={bigPicture.rootRef} className={`client-shell theme-${preferences.appearance} ${preferences.compact ? "dense-collection" : ""} ${bigPicture.active ? "big-picture" : ""}`}>
      <nav className="navigation-rail" aria-label={t("Navegação principal")}>
        <div className="navigation-tabs">{NAVIGATION.filter((item) => !item.hidden).map((item) => <button type="button" key={item.id} aria-current={section === item.id ? "page" : undefined} onClick={() => setSection(item.id)}><span aria-hidden="true">{item.icon}</span> {t(item.label)}</button>)}</div>
        <div className="profile-menu-anchor">
          <button className="rail-avatar" type="button" aria-label={t("Perfil de {name}", {name:profile.displayName})} aria-expanded={profileMenuOpen} aria-controls="profile-actions" onClick={() => setProfileMenuOpen(!profileMenuOpen)}><img className="avatar-img" src="/img/avatar-default.png" alt="" /></button>
          {profileMenuOpen && <div id="profile-actions" className="profile-menu" onKeyDown={(event) => { if (event.key === "Escape") { event.stopPropagation(); setProfileMenuOpen(false); } }}><strong>{profile.displayName}</strong><span className="muted">{t("Conta Google")}</span><button onClick={() => { setSection("profile"); setProfileMenuOpen(false); }}>{t("Meu perfil")}</button><button onClick={() => { setSection("subscription"); setProfileMenuOpen(false); }}>{t("Assinatura")}</button><button onClick={() => { setProfileMenuOpen(false); void onSignOut(); }}>{t("Sair da conta")}</button><button data-menu-close onClick={() => setProfileMenuOpen(false)}>{t("Fechar")}</button></div>}
        </div>
      </nav>
      <header className="client-topbar">
        <img src="/img/logo.png" alt="ShmupX Client" />
        {AMBIENTE === "dev" && <span className="env-badge" title={t("Firestore e login nos emuladores locais (Docker). Nada aqui chega à produção.")}>{t("DESENVOLVIMENTO")}</span>}
        {AMBIENTE === "prd" && import.meta.env.DEV && <span className="env-badge is-production" title={t("Versão em desenvolvimento ligada aos dados reais: o que você fizer vale para os jogadores.")}>{t("DEV · PRODUÇÃO")}</span>}
        <div className="client-status-group">
          {live.state.status === "live" || live.state.status === "preparing" || live.state.status === "connecting" ? <>
            <div className="client-status recording" role="status"><span className="status-dot" />{live.state.status === "live" ? t("AO VIVO") : t("PREPARANDO LIVE")}</div>
            <span className="client-status-caption">{live.state.status === "live" ? `${liveRoom?.title ?? "YouTube"}${liveCountdown ? ` · ${liveCountdown}` : ""}` : t("Conectando ao YouTube…")}</span>
          </> : <>
            <div className={`client-status ${recordingRoom ? "recording" : ""}`} role="status"><span className="status-dot" />{recordingRoom ? t("GRAVANDO RUN") : t("PRONTO")}</div>
            <span className="client-status-caption">{recordingRoom ? t("Gravando · {game}", { game: recordingRoom.title }) : t("Pronto para capturar")}</span>
          </>}
        </div>
        {notifications.version?.update_required && notifications.version.latest && <p className="update-required" role="alert">{t("Atualize o ShmupX Client para a versão {version} para continuar jogando.", { version: notifications.version.latest })}</p>}
        {!autoUpdateActive && !notifications.version?.update_required && notifications.version?.update_available && notifications.version.latest && <p className="update-available">{t("Versão {version} disponível.", { version: notifications.version.latest })}</p>}
        {autoUpdate.state.status === "downloading" && (
          <p className="update-available" role="status">
            {t("Baixando a versão {version}…", { version: autoUpdate.state.version })}
            {autoUpdate.state.progress !== null && ` ${Math.round(autoUpdate.state.progress * 100)}%`}
          </p>
        )}
        {autoUpdate.state.status === "ready" && (
          <p className="update-available" role="status">
            {t("Versão {version} pronta.", { version: autoUpdate.state.version })}{" "}
            <button type="button" className="text-action" onClick={() => void autoUpdate.installNow()} disabled={Boolean(recordingRoom)} title={recordingRoom ? t("Termine a gravação antes de atualizar.") : undefined}>
              {t("Reiniciar e atualizar")}
            </button>
          </p>
        )}
        {autoUpdate.state.status === "installing" && <p className="update-available" role="status">{t("Instalando a versão {version}…", { version: autoUpdate.state.version })}</p>}
        <div className="client-topbar-actions">
          <NotificationBell items={notifications.items} unread={notifications.unread} error={notifications.error} onOpen={openNotification} onSeen={notifications.markSeen} />
          <button type="button" className="secondary-action big-picture-toggle" onClick={bigPicture.toggle} aria-pressed={bigPicture.active} title={t("Alt+Enter")}>{bigPicture.active ? t("Sair do Big Picture") : t("Big Picture")}</button>
        </div>
      </header>
      <div className="client-content">
      {bigPicture.error && <p className="mode-error" role="alert">{t(bigPicture.error)}</p>}
      {section === "submissions" && <SubmissionsPage runs={runs} rooms={rooms} onOpenGame={openGame} />}
      {section === "profile" && <ProfilePage profile={profile} onProfile={onProfile} onSubscription={() => setSection("subscription")} />}
      {section === "subscription" && <SubscriptionPage onEvents={() => setSection("events")} />}
      {section === "home" && <HomePage rooms={rooms} recordingRoomId={recordingRoomId} onNavigate={setSection} onOpenGame={openGame} />}
      {section === "events" && <EventsPage />}
      {section === "ranking" && <RankingPage rooms={rooms} />}
      {section === "settings" && <SettingsPage rooms={rooms} preferences={preferences} onPreferences={(next) => { setPreferences(next); setLanguage(next.language); }} storageError={storageError} onNavigate={setSection} />}
      {section === "help" && <HelpPage />}
      {section === "notification" && <NotificationPage id={notificationId} items={notifications.items} onSelect={setNotificationId} onBack={() => setSection("home")} onRead={notifications.reload} />}
      <div className="client-workspace" hidden={!isLibrary}>
        <RoomTabs rooms={rooms} openRoomIds={openRoomIds} activeRoomId={activeRoomId} recordingRoomId={recordingRoomId} onShowCollection={() => setActiveRoomId(null)} onActivate={activateRoom} onClose={closeRoom} />
        <div className="room-stage">
          {activeRoom === null ? (
            <CollectionPane rooms={rooms} runs={runs} selectedRoomId={selectedRoomId} recordingRoomId={recordingRoomId} onSelectRoom={onSelectRoom} onOpenRoom={openGame} onOpenRanking={(id) => { openGame(id); setRanking(id, true); }} compact={false} bigPicture={bigPicture.active} />
          ) : rankingRoomIds.includes(activeRoom.id) ? (
            <RoomRankingView key={activeRoom.id} room={activeRoom} runs={runs} onBack={() => setRanking(activeRoom.id, false)} />
          ) : (
            renderRoom(activeRoom, { openRanking: () => setRanking(activeRoom.id, true) })
          )}
        </div>
      </div>
      </div>
      <footer className="client-footer"><span><i className={`status-dot ${recordingRoom ? "is-recording" : ""}`} />{recordingRoom ? t("Gravando · {game}", {game:recordingRoom.title}) : t("Nenhuma gravação em andamento")}</span><span>{bigPicture.active ? bigPicture.controller : t("ShmupX Client")} <span className="footer-demo">{t("· dados de demonstração")}</span></span></footer>
    </main>
  );
}
