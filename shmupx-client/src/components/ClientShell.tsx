import { useI18n } from "../i18n";
import { useEffect, useState, type ReactNode } from "react";
import type { GameRoom } from "../types/room";
import { EventsPage, HelpPage, HomePage, RankingPage, type Section } from "./ClientPages";
import { ProfilePage, SubmissionsPage, SubscriptionPage } from "./AccountPages";
import { useBigPicture } from "./useBigPicture";
import type { SessionRun } from "../types/submission";
import CollectionPane from "./CollectionPane";
import SettingsPage from "./SettingsPage";
import { readPreferences, PREFERENCES_KEY } from "./settingsState";
import type { PlayerProfile } from "../auth/firebase";

const NAVIGATION: { id: Section; icon: string; label: string }[] = [
  { id: "home", icon: "⌂", label: "Início" },
  { id: "collection", icon: "▦", label: "Coleção" },
  { id: "events", icon: "◇", label: "Eventos" },
  { id: "ranking", icon: "☆", label: "Ranking" },
  { id: "submissions", icon: "↑", label: "Submissões" },
  { id: "rooms", icon: "☷", label: "Salas" },
  { id: "settings", icon: "⚙", label: "Configurações" },
  { id: "help", icon: "?", label: "Ajuda" },
];
interface ClientShellProps {
  rooms: GameRoom[];
  selectedRoomId: string | null;
  recordingRoomId: string | null;
  onSelectRoom: (id: string) => void;
  children: ReactNode;
  runs: SessionRun[];
  profile: PlayerProfile;
  onProfile: (profile: PlayerProfile) => void;
  onSignOut: () => void;
}

export default function ClientShell({ rooms, selectedRoomId, recordingRoomId, onSelectRoom, children, runs, profile, onProfile, onSignOut }: ClientShellProps) {
  const { t, setLanguage } = useI18n();
  const [preferences, setPreferences] = useState(readPreferences);
  const [section, setSection] = useState<Section>(() => preferences.startup);
  const [storageError, setStorageError] = useState(false);
  const [profileMenuOpen, setProfileMenuOpen] = useState(false);
  const bigPicture = useBigPicture(preferences.startupBigPicture);
  const recordingRoom = rooms.find((room) => room.id === recordingRoomId);

  useEffect(() => {
    try { localStorage.setItem(PREFERENCES_KEY, JSON.stringify(preferences)); setStorageError(false); }
    catch { setStorageError(true); }
  }, [preferences]);

  function openGame(id: string) { onSelectRoom(id); setSection("collection"); }
  const isLibrary = section === "collection" || section === "rooms";

  return (
    <main ref={bigPicture.rootRef} className={`client-shell theme-${preferences.appearance} ${preferences.compact ? "dense-collection" : ""} ${bigPicture.active ? "big-picture" : ""}`}>
      <nav className="navigation-rail" aria-label={t("Navegação principal")}>
        <div className="navigation-tabs">{NAVIGATION.map((item) => <button type="button" key={item.id} aria-current={section === item.id ? "page" : undefined} onClick={() => setSection(item.id)}><span aria-hidden="true">{item.icon}</span> {t(item.label)}</button>)}</div>
        <div className="profile-menu-anchor">
          <button className="rail-avatar" type="button" aria-label={t("Perfil de {name}", {name:profile.displayName})} aria-expanded={profileMenuOpen} aria-controls="profile-actions" onClick={() => setProfileMenuOpen(!profileMenuOpen)}>{profile.displayName.slice(0, 2).toUpperCase()}</button>
          {profileMenuOpen && <div id="profile-actions" className="profile-menu" onKeyDown={(event) => { if (event.key === "Escape") { event.stopPropagation(); setProfileMenuOpen(false); } }}><strong>{profile.displayName}</strong><span className="muted">{t("Conta Google")}</span><button onClick={() => { setSection("profile"); setProfileMenuOpen(false); }}>{t("Meu perfil")}</button><button onClick={() => { setSection("subscription"); setProfileMenuOpen(false); }}>{t("Assinatura")}</button><button onClick={() => { setProfileMenuOpen(false); void onSignOut(); }}>{t("Sair da conta")}</button><button data-menu-close onClick={() => setProfileMenuOpen(false)}>{t("Fechar")}</button></div>}
        </div>
      </nav>
      <header className="client-topbar">
        <img src="/img/logo.png" alt="ShmupX Client" />
        <div className={`client-status ${recordingRoom ? "recording" : ""}`} role="status"><span className="status-dot" />{recordingRoom ? t("GRAVANDO RUN") : t("PRONTO PARA JOGAR")}</div>
        <button type="button" className="secondary-action big-picture-toggle" onClick={bigPicture.toggle} aria-pressed={bigPicture.active} title={t("Alt+Enter")}>{bigPicture.active ? t("Sair do Big Picture") : t("Big Picture")}</button>
      </header>
      <div className="client-content">
      {bigPicture.error && <p className="mode-error" role="alert">{t(bigPicture.error)}</p>}
      {section === "submissions" && <SubmissionsPage runs={runs} rooms={rooms} onOpenGame={openGame} />}
      {section === "profile" && <ProfilePage profile={profile} onProfile={onProfile} onSubscription={() => setSection("subscription")} />}
      {section === "subscription" && <SubscriptionPage onEvents={() => setSection("events")} />}
      {section === "home" && <HomePage rooms={rooms} recordingRoomId={recordingRoomId} onNavigate={setSection} onOpenGame={openGame} />}
      {section === "events" && <EventsPage />}
      {section === "ranking" && <RankingPage rooms={rooms} />}
      {section === "settings" && <SettingsPage preferences={preferences} onPreferences={(next) => { setPreferences(next); setLanguage(next.language); }} storageError={storageError} onNavigate={setSection} />}
      {section === "help" && <HelpPage />}
      <div className="client-workspace" hidden={!isLibrary}>
        <CollectionPane rooms={rooms} runs={runs} selectedRoomId={selectedRoomId} recordingRoomId={recordingRoomId} onSelectRoom={onSelectRoom} compact={section === "rooms"} bigPicture={bigPicture.active} />
        <div className="game-details">{children}</div>
      </div>
      </div>
      <footer className="client-footer"><span><i className={`status-dot ${recordingRoom ? "is-recording" : ""}`} />{recordingRoom ? t("Gravando · {game}", {game:recordingRoom.title}) : t("Nenhuma gravação em andamento")}</span><span>{bigPicture.active ? bigPicture.controller : t("ShmupX Client")} <span className="footer-demo">{t("· dados de demonstração")}</span></span></footer>
    </main>
  );
}
