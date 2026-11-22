import { useState } from "react";
import { useAuth } from "@client/auth/useAuth";
import { useCatalog } from "@client/data/useCatalog";
import { useI18n } from "@client/i18n";
import LoginScreen from "./LoginScreen";
import RoomList from "./RoomList";
import Room from "./Room";
import MisterSettings from "./MisterSettings";

/**
 * ShmupX Lite (Android): só para acompanhar a plataforma pelo celular. Entra nas salas, conversa
 * no chat, vê quem está na sala e assiste às lives. Joga só no MiSTer da mesma rede (abre o jogo na
 * TV, valendo ranking ou treino); PC e transmissão ficam no Client.
 */
export default function App() {
  const { t } = useI18n();
  const auth = useAuth();
  const rooms = useCatalog(auth.state.status === "signed-in");
  const [roomId, setRoomId] = useState<string | null>(null);
  const [settings, setSettings] = useState(false);

  if (auth.state.status === "loading") return <main className="lite-screen lite-center" aria-busy="true"><img src="/img/logo.png" alt="ShmupX" className="lite-logo" /></main>;
  if (auth.state.status === "signed-out") return <LoginScreen busy={auth.state.busy ?? false} error={auth.state.error} onSignIn={auth.signIn} />;

  const profile = auth.state.profile;
  const room = rooms.find((candidate) => candidate.id === roomId);
  if (settings) return <MisterSettings rooms={rooms} onBack={() => setSettings(false)} />;
  if (room) return <Room room={room} profile={profile} onBack={() => setRoomId(null)} onSettings={() => setSettings(true)} />;
  return (
    <RoomList
      rooms={rooms}
      profileName={profile.displayName}
      onOpen={setRoomId}
      onSettings={() => setSettings(true)}
      onSignOut={() => { if (window.confirm(t("Sair da conta?"))) void auth.signOut(); }}
    />
  );
}
