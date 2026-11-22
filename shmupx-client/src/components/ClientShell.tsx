import { useI18n } from "../i18n";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@tauri-apps/api/core";
import type { GameRoom } from "../types/room";
import { EventsPage, HelpPage, HomePage, RankingPage, type Section } from "./ClientPages";
import { SubmissionsPage, SubscriptionPage } from "./AccountPages";
import { useBigPicture } from "./useBigPicture";
import type { SessionRun } from "../types/submission";
import CollectionPane from "./CollectionPane";
import RoomRankingView from "./RoomRankingView";
import { useDragReorder } from "./useDragReorder";
import SettingsPage from "./SettingsPage";
import { readPreferences, PREFERENCES_KEY } from "./settingsState";
import NotificationBell from "./NotificationBell";
import NotificationPage from "./NotificationPage";
import { useNotifications } from "../data/useNotifications";
import { useAutoUpdate } from "../data/useAutoUpdate";
import { useCountdown, useLive } from "../data/useLive";
import { AMBIENTE, type PlayerProfile } from "../auth/firebase";
import { leaveRoomPresence, usePresenceHeartbeat, videoIdFromUrl } from "../data/useRoomPresence";
import { clearRoomSession, roomJoinedAt } from "../data/roomSession";
import { useDialogFocus } from "./useDialogFocus";
import { DEFAULT_AVATAR, useMyPublicProfile } from "../data/publicProfile";
import { savePresenceMode, usePresenceMode } from "../data/presenceMode";
import { visibleDialog } from "./navigationInput";

/** Salas abertas nas abas, por jogador: fechar o app não tira o jogador da sala. */
const openRoomsKey = (uid: string) => `shmupx.client.openRooms.v1.${uid}`;
function readOpenRooms(uid: string): string[] {
  try {
    const value = JSON.parse(localStorage.getItem(openRoomsKey(uid)) ?? "[]");
    return Array.isArray(value) ? value.filter((id): id is string => typeof id === "string") : [];
  } catch {
    return [];
  }
}

interface ClientShellProps {
  rooms: GameRoom[];
  selectedRoomId: string | null;
  recordingRoomId: string | null;
  onSelectRoom: (id: string) => void;
  /** Conteúdo da sala de um jogo; `openRanking` troca a aba para o ranking do jogo. */
  renderRoom: (room: GameRoom, actions: { openRanking: () => void; showCollection: () => void }) => ReactNode;
  runs: SessionRun[];
  profile: PlayerProfile;
  onProfile: (profile: PlayerProfile) => void;
  onSignOut: () => void;
}

export default function ClientShell({ rooms, selectedRoomId, recordingRoomId, onSelectRoom, renderRoom, runs, profile, onProfile, onSignOut }: ClientShellProps) {
  const { t, setLanguage } = useI18n();
  const [preferences, setPreferences] = useState(readPreferences);
  // O app sempre abre na Coleção (a tela principal).
  const [section, setSection] = useState<Section>("collection");
  const [storageError, setStorageError] = useState(false);
  const [profileMenuOpen, setProfileMenuOpen] = useState(false);
  // Configurações numa janela (engrenagem acima do avatar); o avatar só tem status e sair.
  const [settingsOpen, setSettingsOpen] = useState(false);
  const settingsDialogRef = useDialogFocus(settingsOpen, () => setSettingsOpen(false));
  const presenceMode = usePresenceMode();
  const myPublic = useMyPublicProfile(profile.displayName);
  const profileMenuRef = useRef<HTMLDivElement>(null);
  const profileButtonRef = useRef<HTMLButtonElement>(null);
  // O menu do perfil fecha com um clique fora dele (o botão do avatar alterna sozinho).
  useEffect(() => {
    if (!profileMenuOpen) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (profileMenuRef.current?.contains(target) || profileButtonRef.current?.contains(target)) return;
      setProfileMenuOpen(false);
    };
    window.addEventListener("pointerdown", onPointerDown, true);
    return () => window.removeEventListener("pointerdown", onPointerDown, true);
  }, [profileMenuOpen]);
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
  const [openRoomIds, setOpenRoomIds] = useState<string[]>(() => readOpenRooms(profile.uid));
  const [activeRoomId, setActiveRoomId] = useState<string | null>(null);
  const [rankingRoomIds, setRankingRoomIds] = useState<string[]>([]);
  const activeRoom = rooms.find((room) => room.id === activeRoomId) ?? null;
  // Menu lateral = os jogos abertos, na mesma ordem das abas; arrastar ali reordena as abas também.
  const railDrag = useDragReorder({ ids: openRoomIds, onReorder: setOpenRoomIds, axis: "y" });

  useEffect(() => {
    try { localStorage.setItem(openRoomsKey(profile.uid), JSON.stringify(openRoomIds)); } catch { /* fica só nesta sessão */ }
  }, [profile.uid, openRoomIds]);
  // Presença em todas as salas abertas: online na que está na tela, away nas outras.
  const liveVideoId = live.state.status === "live" ? videoIdFromUrl(live.state.watch_url) : null;
  usePresenceHeartbeat(profile, openRoomIds, section === "collection" ? activeRoomId : null, recordingRoomId,
    liveVideoId && live.state.game_id ? { roomId: live.state.game_id, videoId: liveVideoId } : null, presenceMode === "away");
  // Fechar a aba sai da sala (chat e lista de jogadores): pede confirmação, o × é fácil de clicar sem querer.
  const [closingRoomId, setClosingRoomId] = useState<string | null>(null);
  const closingRoom = rooms.find((room) => room.id === closingRoomId) ?? null;
  const closeDialogRef = useDialogFocus(closingRoom !== null, () => setClosingRoomId(null));

  // Um jogo despublicado sai do catálogo: a aba dele fecha. Catálogo ainda vazio (carregando) não
  // fecha nada: as abas guardadas voltariam vazias.
  useEffect(() => {
    if (rooms.length === 0) return;
    const valid = (id: string) => rooms.some((room) => room.id === id);
    setOpenRoomIds((current) => (current.every(valid) ? current : current.filter(valid)));
    setActiveRoomId((current) => (current === null || valid(current) ? current : null));
  }, [rooms]);

  function openGame(id: string) {
    roomJoinedAt(profile.uid, id);
    setOpenRoomIds((current) => (current.includes(id) ? current : [...current, id]));
    setActiveRoomId(id);
    onSelectRoom(id);
    setSection("collection");
  }
  function activateRoom(id: string) { setActiveRoomId(id); onSelectRoom(id); }
  function closeRoom(id: string) {
    void leaveRoomPresence(id, profile.uid);
    // Saiu da sala: ao voltar, o chat começa limpo.
    clearRoomSession(profile.uid, id);
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
  // Versão instalada (tauri.conf.json), no rodapé: acompanha cada atualização sozinha.
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => { if (isTauri()) getVersion().then(setVersion).catch(() => {}); }, []);

  // Menu de navegação no topo: "Minhas coleções" fixo; na sala, o jogo; no ranking do jogo, "Ranking".
  function showCollection() { setSection("collection"); setActiveRoomId(null); }
  function showRoom(id: string) { setSection("collection"); setRanking(id, false); activateRoom(id); }
  const crumbRoom = isLibrary ? activeRoom : null;
  const crumbs: { label: string; onClick?: () => void }[] = [{ label: t("Minhas coleções"), onClick: showCollection }];
  if (crumbRoom) {
    crumbs.push({ label: crumbRoom.title, onClick: () => showRoom(crumbRoom.id) });
    if (rankingRoomIds.includes(crumbRoom.id)) crumbs.push({ label: t("Ranking") });
  }

  // Esc volta um nível no menu de navegação: Ranking → sala → Minhas coleções; telas internas
  // (Configurações, perfil…) → Minhas coleções. Não age com janela aberta (o Esc a fecha), ao
  // digitar nem no Big Picture (lá o Esc sai do modo).
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key !== "Escape" || event.defaultPrevented || bigPicture.active || visibleDialog()) return;
      const focused = document.activeElement;
      if (focused instanceof HTMLElement && focused.matches("input,select,textarea,[contenteditable=true]")) return;
      if (section !== "collection") { setSection("collection"); setActiveRoomId(null); }
      else if (activeRoomId && rankingRoomIds.includes(activeRoomId)) setRanking(activeRoomId, false);
      else if (activeRoomId) setActiveRoomId(null);
      else return;
      event.preventDefault();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [section, activeRoomId, rankingRoomIds, bigPicture.active]);

  return (
    <main ref={bigPicture.rootRef} className={`client-shell theme-${preferences.appearance} ${preferences.compact ? "dense-collection" : ""} ${bigPicture.active ? "big-picture" : ""}`}>
      <nav className="navigation-rail" aria-label={t("Navegação principal")}>
        {/* Menu lateral: só os jogos abertos (os mesmos das abas da sala). A Coleção, o Dashboard e as
            Configurações ficam fora daqui (aba "← Coleção" e menu do perfil). */}
        <div ref={railDrag.listRef} className="navigation-tabs">
          {openRoomIds.map((id) => {
            const room = rooms.find((candidate) => candidate.id === id);
            if (!room) return null;
            // O item (não girado) é o que se arrasta; o botão dentro dele é girado 180° (texto vertical).
            return <div key={id} ref={railDrag.register(id)} className={`rail-room-item ${railDrag.draggingId === id ? "is-dragging" : ""}`} style={railDrag.styleFor(id)}
              onPointerDown={(event) => railDrag.onPointerDown(event, id, ".rail-room-close")}
              // Botão do meio (scroll) fecha a sala, como o ×; o mousedown evita a rolagem automática.
              onMouseDown={(event) => { if (event.button === 1) event.preventDefault(); }}
              onAuxClick={(event) => { if (event.button === 1) { event.preventDefault(); setClosingRoomId(id); } }}>
              <button type="button" className="rail-room" title={room.title} aria-current={section === "collection" && activeRoomId === id ? "page" : undefined}
                onClick={() => { if (railDrag.consumeClick()) return; setSection("collection"); activateRoom(id); }}>
                {recordingRoomId === id && <span className="rail-room-recording" aria-label={t("Gravando")} />}
                <span className="rail-room-title">{room.title}</span>
              </button>
              <button type="button" className="rail-room-close" aria-label={t("Fechar sala de {game}", { game: room.title })} title={t("Fechar")} onClick={() => setClosingRoomId(id)}>×</button>
            </div>;
          })}
        </div>
        <div className="profile-menu-anchor">
          <button type="button" className="rail-settings" aria-label={t("Configurações")} title={t("Configurações")} aria-haspopup="dialog" onClick={() => { setProfileMenuOpen(false); setSettingsOpen(true); }}>
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><circle cx="12" cy="12" r="3.2" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.6 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.6-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" /></svg>
          </button>
          <button ref={profileButtonRef} className="rail-avatar" type="button" aria-label={t("Status de {name}", {name:profile.displayName})} aria-expanded={profileMenuOpen} aria-controls="profile-actions" onClick={() => setProfileMenuOpen(!profileMenuOpen)}>
            <img className="avatar-img" src={myPublic.profile?.avatar || DEFAULT_AVATAR} alt="" />
            <span className={`presence-dot rail-status is-${presenceMode}`} aria-hidden="true" />
          </button>
          {profileMenuOpen && <div id="profile-actions" ref={profileMenuRef} className="profile-menu" role="menu" onKeyDown={(event) => { if (event.key === "Escape") { event.stopPropagation(); setProfileMenuOpen(false); } }}>
            <strong>{profile.displayName}</strong><span className="muted">{myPublic.profile ? `@${myPublic.profile.handle}` : t("Conta Google")}</span>
            {([["online", "Online"], ["away", "Ausente"]] as const).map(([mode, label]) => (
              <button key={mode} role="menuitemradio" aria-checked={presenceMode === mode} className="presence-option" onClick={() => { savePresenceMode(mode); setProfileMenuOpen(false); }}>
                <span className={`presence-dot is-${mode}`} aria-hidden="true" /> {t(label)}{presenceMode === mode && <span aria-hidden="true"> ✓</span>}
              </button>
            ))}
            <button role="menuitem" onClick={() => { setProfileMenuOpen(false); void onSignOut(); }}>{t("Sair da conta")}</button>
          </div>}
        </div>
      </nav>
      <header className="client-topbar">
        <nav className="topbar-breadcrumb" aria-label={t("Você está em")}>
          <ol>
            {crumbs.map((crumb, index) => {
              const last = index === crumbs.length - 1;
              return (
                <li key={index}>
                  {last || !crumb.onClick
                    ? <span aria-current={last ? "page" : undefined} title={crumb.label}>{crumb.label}</span>
                    : <button type="button" onClick={crumb.onClick} title={crumb.label}>{crumb.label}</button>}
                </li>
              );
            })}
          </ol>
        </nav>
        {AMBIENTE === "dev" && <span className="env-badge" title={t("Firestore e login nos emuladores locais (Docker). Nada aqui chega à produção.")}>{t("DESENVOLVIMENTO")}</span>}
        {AMBIENTE === "prd" && import.meta.env.DEV && <span className="env-badge is-production" title={t("Versão em desenvolvimento ligada aos dados reais: o que você fizer vale para os jogadores.")}>{t("DEV · PRODUÇÃO")}</span>}
        <div className="client-status-group">
          {live.state.status === "live" || live.state.status === "waiting" || live.state.status === "preparing" || live.state.status === "connecting" ? <>
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
      {/* Telas do menu do perfil: voltar para a Coleção (a tela principal). */}
      {section === "submissions" && <SubmissionsPage runs={runs} rooms={rooms} onOpenGame={openGame} />}
      {section === "subscription" && <SubscriptionPage onEvents={() => setSection("events")} />}
      {section === "home" && <HomePage rooms={rooms} recordingRoomId={recordingRoomId} onNavigate={setSection} onOpenGame={openGame} />}
      {section === "events" && <EventsPage />}
      {section === "ranking" && <RankingPage rooms={rooms} />}
      {settingsOpen && (
        <div className="controller-dialog-backdrop">
          <div ref={settingsDialogRef} role="dialog" aria-modal="true" aria-label={t("Configurações")} tabIndex={-1} className="controller-dialog settings-modal">
            <header>
              <div><h2>{t("Configurações")}</h2></div>
              <button type="button" className="secondary-action" onClick={() => setSettingsOpen(false)}>{t("Fechar")}</button>
            </header>
            <div className="controller-dialog-body">
              <SettingsPage rooms={rooms} preferences={preferences} onPreferences={(next) => { setPreferences(next); setLanguage(next.language); }} storageError={storageError}
                onNavigate={(next) => { setSettingsOpen(false); setSection(next); }} profile={profile} onProfile={onProfile} />
            </div>
          </div>
        </div>
      )}
      {section === "help" && <HelpPage />}
      {section === "notification" && <NotificationPage id={notificationId} items={notifications.items} onSelect={setNotificationId} onBack={() => setSection("home")} onRead={notifications.reload} />}
      {/* Sala aberta (sem o ranking): o painel de jogadores sobe até a altura das abas. */}
      <div className="client-workspace" hidden={!isLibrary}>
        {/* As abas de cima saíram por enquanto: os jogos abertos ficam no menu lateral. */}
        <div className="room-stage">
          {activeRoom === null ? (
            <CollectionPane rooms={rooms} runs={runs} selectedRoomId={selectedRoomId} recordingRoomId={recordingRoomId} onSelectRoom={onSelectRoom} onOpenRoom={openGame} onOpenRanking={(id) => { openGame(id); setRanking(id, true); }} compact={false} bigPicture={bigPicture.active} />
          ) : rankingRoomIds.includes(activeRoom.id) ? (
            <RoomRankingView key={activeRoom.id} room={activeRoom} runs={runs} onBack={() => setRanking(activeRoom.id, false)} />
          ) : (
            renderRoom(activeRoom, { openRanking: () => setRanking(activeRoom.id, true), showCollection: () => setActiveRoomId(null) })
          )}
        </div>
      </div>
      </div>
      {closingRoom && (
        <div className="controller-dialog-backdrop">
          <div ref={closeDialogRef} role="dialog" aria-modal="true" aria-labelledby="close-room-title" tabIndex={-1} className="controller-dialog close-room-dialog">
            <header>
              <div>
                <h2 id="close-room-title">{t("Sair da sala de {game}?", { game: closingRoom.title })}</h2>
                <p>{t("Você sai do chat e da lista de jogadores desta sala. Para voltar, abra o jogo de novo na coleção.")}</p>
                {recordingRoomId === closingRoom.id && <p className="mode-error" role="alert">{t("Há uma run sendo gravada nesta sala: a gravação continua até você parar.")}</p>}
              </div>
            </header>
            <footer>
              <button type="button" className="secondary-action" onClick={() => setClosingRoomId(null)}>{t("Cancelar")}</button>
              <button type="button" className="primary-action" onClick={() => { closeRoom(closingRoom.id); setClosingRoomId(null); }}>{t("Sair da sala")}</button>
            </footer>
          </div>
        </div>
      )}
      <footer className="client-footer"><span><i className={`status-dot ${recordingRoom ? "is-recording" : ""}`} />{recordingRoom ? t("Gravando · {game}", {game:recordingRoom.title}) : t("Nenhuma gravação em andamento")}</span><span>{bigPicture.active && <>{bigPicture.controller} · </>}<span className="footer-version">SHMUPX CLIENT{version && ` v${version}`}</span></span></footer>
    </main>
  );
}
