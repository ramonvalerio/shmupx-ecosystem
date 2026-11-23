import { useState } from "react";
import { useAuth } from "@client/auth/useAuth";
import { useCatalog } from "@client/data/useCatalog";
import { useI18n } from "@client/i18n";
import LoginScreen from "./LoginScreen";
import RoomList from "./RoomList";
import Room from "./Room";
import MisterSettings from "./MisterSettings";
import { PlayProvider } from "./play";
import { usePreset, useTheme } from "./prefs";
import { useBackLayer } from "./back";

/**
 * ShmupX Lite (Android): entra nas salas, conversa no chat, vê quem está na sala e assiste às
 * lives. Joga pela lista (▶, com a configuração da tela inicial) ou pela sala (Play Training /
 * Play Ranked): no MiSTer da mesma rede (na TV) ou no PC, pedindo ao ShmupX Client aberto na mesma
 * conta (Steam e emulador). A transmissão fica no Client.
 */
export default function App() {
  const { t } = useI18n();
  const auth = useAuth();
  const rooms = useCatalog(auth.state.status === "signed-in");
  const [roomId, setRoomId] = useState<string | null>(null);
  const [settings, setSettings] = useState(false);
  const [preset, setPreset] = usePreset();
  const [theme, setTheme] = useTheme();
  // Voltar do Android: fecha os ajustes sem fechar o app (a sala cuida da própria saída).
  useBackLayer(settings, () => setSettings(false));

  if (auth.state.status === "loading") return <main className="lite-screen lite-center" aria-busy="true"><img src="/img/logo.png" alt="ShmupX" className="lite-logo" /></main>;
  if (auth.state.status === "signed-out") return <LoginScreen busy={auth.state.busy ?? false} error={auth.state.error} onSignIn={auth.signIn} />;

  const profile = auth.state.profile;
  const room = rooms.find((candidate) => candidate.id === roomId);
  let screen;
  if (settings) screen = <MisterSettings rooms={rooms} theme={theme} onTheme={setTheme} onBack={() => setSettings(false)} />;
  else if (room) screen = <Room room={room} profile={profile} preset={preset} onBack={() => setRoomId(null)} onSettings={() => setSettings(true)} />;
  else screen = (
    <RoomList
      rooms={rooms}
      profileName={profile.displayName}
      preset={preset}
      onPreset={setPreset}
      onOpen={setRoomId}
      onSettings={() => setSettings(true)}
      onSignOut={() => { if (window.confirm(t("Sair da conta?"))) void auth.signOut(); }}
    />
  );
  return <PlayProvider uid={profile.uid} onSettings={() => setSettings(true)}>{screen}</PlayProvider>;
}
