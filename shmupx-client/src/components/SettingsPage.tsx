import { useI18n } from "../i18n";
import { useState, type ReactNode } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { appDataDir } from "@tauri-apps/api/path";
import { PageHeading, type Section } from "./ClientPages";
import { defaultPreferences, type ClientPreferences } from "./settingsState";
import ConfigureControllerDialog from "./ConfigureControllerDialog";
import { readMappings } from "./controllerMappings";
import ImportGamesDialog from "./ImportGamesDialog";
import MisterConnect from "./MisterConnect";
import ProfileSettings from "./ProfileSettings";
import type { PlayerProfile } from "../auth/firebase";
import { useYouTube } from "../data/useLive";
import { clearSteamLibrary, useSteamLibrary } from "../data/steamLibrary";
import { romsFor, useEmulatorLibrary } from "../data/emulatorLibrary";
import type { GameRoom } from "../types/room";

const GROUPS = [
  { id: "profile", title: "Perfil", detail: "Avatar, nome, nick e assinatura", icon: "◉" },
  { id: "connections", title: "Importar jogos", detail: "Bibliotecas, plataformas e MiSTer FPGA", icon: "▦" },
  { id: "integrations", title: "Integrações", detail: "YouTube", icon: "▷" },
  { id: "controller", title: "Controle", detail: "Dispositivo e comandos", icon: "✥" },
  { id: "storage", title: "Armazenamento", detail: "Destino das gravações", icon: "▣" },
  { id: "recording", title: "Gravação", detail: "Vídeo e áudio", icon: "●" },
  { id: "capture", title: "Captura", detail: "Fonte da imagem", icon: "▤" },
  { id: "interface", title: "Interface", detail: "Visual e abertura do cliente", icon: "◐" },
] as const;

function SettingsCard({ title, detail, children }: { title: string; detail?: string; children: ReactNode }) {
  const { t } = useI18n();
  return <section className="dashboard-panel settings-card"><h2>{t(title)}</h2>{detail && <p className="muted">{t(detail)}</p>}{children}</section>;
}

/** YouTube do jogador: conectar o canal para transmitir ao vivo (START LIVE na sala). */
function YouTubeCard() {
  const { t } = useI18n();
  const { connection, busy, error, connect, disconnect } = useYouTube();
  const connected = Boolean(connection?.connected);
  return <SettingsCard title={t("Integrações")} detail={t("Conecte serviços externos à sua conta ShmupX.")}>
    <div className="integration-row">
      <span className="integration-icon" aria-hidden="true">▷</span>
      <div className="integration-info">
        <strong>{t("YouTube")}</strong>
        <small>{connection === null ? t("Verificando…") : connected ? t("Conectado como {channel}", { channel: connection?.channel_title ?? "YouTube" }) : t("Não conectado. Conecte para transmitir suas partidas ao vivo.")}</small>
      </div>
      {connected
        ? <button className="secondary-action" disabled={busy} onClick={() => void disconnect()}>{busy ? t("Aguarde…") : t("Desconectar")}</button>
        : <button className="primary-action" disabled={busy || connection === null} onClick={() => void connect()}>{busy ? t("Aguardando o Google…") : t("Conectar")}</button>}
    </div>
    {busy && !connected && <p className="muted" role="status">{t("Conclua a autorização no navegador que foi aberto e volte para o aplicativo.")}</p>}
    {error && <p className="mode-error" role="alert">{t(error)}</p>}
    <p className="muted">{t("O ShmupX pede permissão para criar e encerrar transmissões no seu canal. As lives são não listadas, a menos que você marque \"Live pública\" na sala.")}</p>
  </SettingsCard>;
}

/** Uma biblioteca conectada (Steam, Fightcade, Local): situação, contagem e ações. */
function LibraryRow({ icon, name, connected, soon, games, installed, installedLabel, syncedAt, folder, busy, onConnect, onSync, onDisconnect }: {
  icon: ReactNode; name: string; connected: boolean; soon?: boolean; games?: number; installed?: number; installedLabel?: string; syncedAt?: number; folder?: string; busy?: boolean;
  onConnect?: () => void; onSync?: () => void; onDisconnect?: () => void;
}) {
  const { t, locale } = useI18n();
  const date = syncedAt ? new Date(syncedAt).toLocaleString(locale, { dateStyle: "short", timeStyle: "short" }) : null;
  return <div className="library-row">
    <div className="library-row-head">
      <span className="library-row-icon" aria-hidden="true">{icon}</span>
      <div className="library-row-name">
        <strong>{name}</strong>
        <small className={connected ? "is-connected" : ""}>{connected ? `✓ ${t("Conectada")}` : soon ? t("Em breve") : t("Não conectada")}</small>
      </div>
      <div className="library-row-actions">
        {connected
          ? <><button className="secondary-action" disabled={busy} onClick={onSync}>{t(busy ? "Sincronizando…" : "Sincronizar")}</button><button className="secondary-action" disabled={busy} onClick={onDisconnect}>{t("Desconectar")}</button></>
          : <button className="secondary-action" disabled={soon || busy} onClick={onConnect} title={soon ? t("A leitura desta biblioteca ainda não está disponível nesta versão.") : undefined}>{t(busy ? "Conectando…" : "Conectar")}</button>}
      </div>
    </div>
    {connected && <div className="library-row-stats">
      <span>{installedLabel ? t("{games} jogos do ShmupX · {installed} ROMs", { games: games ?? 0, installed: installed ?? 0 }) : t("{games} jogos · {installed} instalados", { games: games ?? 0, installed: installed ?? 0 })}</span>
      {folder && <span>{t("Pasta")} <span className="muted">{folder}</span></span>}
      {date && <span>{t("Última sincronização")} <span className="muted">{date}</span></span>}
    </div>}
  </div>;
}

const IconGamepad = () => <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M6 8h12a4 4 0 0 1 4 4v1a4 4 0 0 1-7 2.6L14 15h-4l-1 .6A4 4 0 0 1 2 13v-1a4 4 0 0 1 4-4z" /><path d="M7 11v3M5.5 12.5h3" /><circle cx="16" cy="11.5" r=".6" fill="currentColor" /><circle cx="18" cy="13.5" r=".6" fill="currentColor" /></svg>;
const IconFolder = () => <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round"><path d="M3 6.5A1.5 1.5 0 0 1 4.5 5h4.2l2 2.2h8.8A1.5 1.5 0 0 1 21 8.7v9.8a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z" /></svg>;

/** Configurações › Importar jogos: bibliotecas conectadas (como no Client antigo). */
function ConnectedLibraries({ rooms, onImport }: { rooms: GameRoom[]; onImport: () => void }) {
  const { t } = useI18n();
  const steam = useSteamLibrary();
  const emulator = useEmulatorLibrary();
  const [emuBusy, setEmuBusy] = useState(false);
  // Jogos do ShmupX que têm ROM na biblioteca do emulador.
  const emuGames = emulator.library ? rooms.filter((room) => romsFor(room).some((rom) => emulator.library!.roms.includes(rom))).length : 0;
  async function syncEmulator() {
    setEmuBusy(true);
    setError(null);
    try {
      const result = await emulator.refresh();
      if (!result.found) setError(t("Não encontramos o Fightcade neste computador. Instale o Fightcade e tente de novo."));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setEmuBusy(false);
    }
  }
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function syncSteam() {
    setBusy(true);
    setError(null);
    try {
      const result = await steam.refresh();
      if (!result.found) setError(t("Não encontramos a Steam neste computador. Instale a Steam e tente de novo."));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }
  return <SettingsCard title={t("Bibliotecas conectadas")} detail={t("Conecte suas plataformas para importar seus jogos automaticamente.")}>
    <div className="library-rows">
      <LibraryRow icon={<IconGamepad />} name="Steam" connected={Boolean(steam.library)} games={steam.library?.total} installed={steam.library?.appIds.length} syncedAt={steam.library?.importedAt} busy={busy}
        onConnect={() => void syncSteam()} onSync={() => void syncSteam()} onDisconnect={clearSteamLibrary} />
      <LibraryRow icon={<IconFolder />} name={t("Emulador")} connected={Boolean(emulator.library)} games={emuGames} installed={emulator.library?.roms.length} installedLabel="ROMs" syncedAt={emulator.library?.importedAt} folder={emulator.library?.root} busy={emuBusy}
        onConnect={() => void syncEmulator()} onSync={() => void syncEmulator()} onDisconnect={emulator.disconnect} />
      <LibraryRow icon={<IconFolder />} name={t("Local")} connected={false} soon />
    </div>
    {error && <p className="mode-error" role="alert">{error}</p>}
    <button className="primary-action library-import" onClick={onImport}><span aria-hidden="true">+</span> {t("Importar jogos")}</button>
  </SettingsCard>;
}

export default function SettingsPage({ rooms = [], preferences, onPreferences, storageError, onNavigate, profile, onProfile }: {
  rooms?: GameRoom[]; preferences: ClientPreferences; onPreferences: (value: ClientPreferences) => void; storageError: boolean; onNavigate: (section: Section) => void;
  profile: PlayerProfile; onProfile: (profile: PlayerProfile) => void;
}) {
  const { t } = useI18n();
  const [group, setGroup] = useState<typeof GROUPS[number]["id"]>("profile");
  const [folder, setFolder] = useState("");
  const [folderBusy, setFolderBusy] = useState(false);
  const [controllers, setControllers] = useState<{ id: string; standard: boolean }[]>([]);
  const [controllerMessage, setControllerMessage] = useState("");
  const [configureController,setConfigureController] = useState(false);
  const [importOpen,setImportOpen] = useState(false);
  function detectControllers() {
    try {
      const devices = navigator.getGamepads ? [...navigator.getGamepads()].filter((pad): pad is Gamepad => pad !== null && pad.connected) : [];
      setControllers(devices.map((pad) => ({id:pad.id, standard:pad.mapping === "standard" || readMappings().some((mapping)=>mapping.deviceId===pad.id)})));
      setControllerMessage(devices.length ? "controllers-detected" : "Nenhum controle detectado. Conecte-o e pressione um botão antes de consultar novamente.");
    } catch { setControllers([]); setControllerMessage("A leitura de controles não está disponível neste ambiente."); }
  }
  async function locateFolder() {
    if (!isTauri()) { setFolder("A pasta de gravações só pode ser consultada no aplicativo desktop."); return; }
    setFolderBusy(true);
    try { setFolder(await appDataDir()); } catch { setFolder("Não foi possível consultar a pasta de dados do aplicativo."); } finally { setFolderBusy(false); }
  }
  return <div className="client-page settings-page">
    {importOpen && <ImportGamesDialog rooms={rooms} onClose={()=>setImportOpen(false)} onViewCollection={()=>onNavigate("collection")} />}
    {configureController && <ConfigureControllerDialog onClose={()=>{setConfigureController(false);detectControllers();}} />}
    <PageHeading title={t("Configurações")} detail={t("Biblioteca, captura e preferências do cliente.")} />
    <div className="settings-layout">
      <nav className="settings-menu" aria-label={t("Grupos de configurações")}>{GROUPS.map((item) => <button key={item.id} aria-current={group === item.id ? "page" : undefined} onClick={() => setGroup(item.id)}><span aria-hidden="true">{item.icon}</span><span><strong>{t(item.title)}</strong><small>{t(item.detail)}</small></span></button>)}</nav>
      <div className="settings-panels">
        {group === "profile" && <SettingsCard title={t("Perfil")}><ProfileSettings profile={profile} onProfile={onProfile} /></SettingsCard>}
        {group === "interface" && <>
          <SettingsCard title={t("Idioma do sistema")} detail={t("O idioma muda na hora e fica salvo neste dispositivo.")}><label className="settings-field">{t("Idioma do sistema")}<select value={preferences.language} onChange={(event) => onPreferences({ ...preferences, language: event.target.value === "en" ? "en" : "pt-BR" })}><option value="pt-BR">{t("Português (Brasil)")}</option><option value="en">{t("English")}</option></select></label></SettingsCard>
          <SettingsCard title={t("Visual")} detail={t("Escolha a paleta do ShmupX. A mudança vale na hora e fica guardada para a próxima vez.")}>
            <div className="theme-options" role="group" aria-label={t("Visual")}>{([{ id: "default", label: "Padrão" }, { id: "dark", label: "Escuro" }, { id: "light", label: "Claro" }] as const).map((theme) => <button key={theme.id} aria-pressed={preferences.appearance === theme.id} onClick={() => onPreferences({ ...preferences, appearance: theme.id })}><span className={`theme-swatch swatch-${theme.id}`} aria-hidden="true" />{t(theme.label)}</button>)}</div>
            <label className="checkbox-field"><input type="checkbox" checked={preferences.compact} onChange={(event) => onPreferences({ ...preferences, compact: event.target.checked })} />{t("Coleção compacta")}</label>
          </SettingsCard>
          <SettingsCard title={t("Abrir ao iniciar")} detail={t("O cliente sempre abre na Coleção. Escolha o modo de apresentação.")}>
            <label className="checkbox-field"><input type="checkbox" checked={preferences.startupBigPicture} onChange={(event) => onPreferences({ ...preferences, startupBigPicture: event.target.checked })} />{t("Abrir em modo Big Picture")}</label>
            <p className="muted">{t("No desktop, Big Picture usa tela cheia. Na prévia do navegador, altera a apresentação.")}</p>
          </SettingsCard>
          <SettingsCard title={t("Preferências da interface")}><p className="muted" role="status">{storageError ? t("Preferências aplicadas nesta sessão. Não foi possível salvar neste dispositivo.") : t("As alterações são aplicadas e salvas automaticamente neste dispositivo.")}</p><button className="secondary-action" onClick={() => onPreferences(defaultPreferences())}>{t("Restaurar preferências da interface")}</button></SettingsCard>
        </>}
        {group === "connections" && <><ConnectedLibraries rooms={rooms} onImport={() => setImportOpen(true)} /><SettingsCard title={t("MiSTer FPGA")} detail={t("Jogue no MiSTer e as partidas entram no seu perfil e no ranking, como as do PC.")}><MisterConnect /></SettingsCard></>}
        {group === "integrations" && <YouTubeCard />}
        {group === "controller" && <><SettingsCard title={t("Dispositivo de entrada")} detail={t("Consulte os controles que este ambiente disponibiliza ao cliente.")}><button className="secondary-action" onClick={detectControllers}>{t("Detectar controles")}</button><p className="muted" role="status">{controllerMessage === "controllers-detected" ? t("{count} controle(s) detectado(s).",{count:controllers.length}) : t(controllerMessage)}</p>{controllers.length > 0 && <ul className="controller-devices">{controllers.map((device, index) => <li key={`${index}:${device.id}`}>{device.id} · {t(device.standard ? "compatível com Big Picture" : "mapeamento não padrão")}</li>)}</ul>}</SettingsCard><SettingsCard title={t("Comandos do Big Picture")}><dl className="settings-facts"><dt>{t("Navegar")}</dt><dd>{t("Direcional ou analógico esquerdo")}</dd><dt>{t("Confirmar")}</dt><dd>{t("A")}</dd><dt>{t("Voltar / fechar")}</dt><dd>{t("B")}</dd><dt>{t("Teclado")}</dt><dd>{t("Setas, Enter e Esc")}</dd><dt>{t("Alternar Big Picture")}</dt><dd>{t("Alt+Enter")}</dd><dt>{t("Comunicação por voz no desktop")}</dt><dd>{t("Select / View / Back")}</dd></dl><button className="primary-action" onClick={()=>setConfigureController(true)}>{t("Configurar controle")}</button><p className="muted">{t("O perfil personalizado afeta apenas a navegação do Big Picture.")}</p></SettingsCard></>}
        {group === "storage" && <SettingsCard title={t("Destino das gravações")} detail={t("As runs são salvas na pasta de dados do aplicativo.")}><button className="secondary-action" disabled={folderBusy} onClick={locateFolder}>{folderBusy ? t("Consultando…") : t("Consultar pasta atual")}</button>{folder && <p className="folder-result" role="status">{t(folder)}</p>}<p className="muted">{t("A seleção de outra pasta e a listagem de replays ainda não estão disponíveis.")}</p><button className="text-action" onClick={() => onNavigate("submissions")}>{t("Ver runs desta sessão →")}</button></SettingsCard>}
        {group === "recording" && <><SettingsCard title={t("Vídeo")} detail={t("Parâmetros atuais do gravador, somente para consulta.")}><dl className="settings-facts"><dt>{t("Taxa de quadros")}</dt><dd>{t("60 fps")}</dd><dt>{t("Codificador")}</dt><dd>{t("H.264")}</dd><dt>{t("Preset")}</dt><dd>{t("Ultrafast")}</dd><dt>{t("Formato")}</dt><dd>{t("MP4")}</dd></dl></SettingsCard><SettingsCard title={t("Áudio")}><dl className="settings-facts"><dt>{t("Fonte")}</dt><dd>{t("Áudio do sistema")}</dd><dt>{t("Captura")}</dt><dd>{t("Dispositivo de loopback")}</dd></dl><p className="muted">{t("A seleção do dispositivo e a edição dos parâmetros ainda não estão disponíveis.")}</p></SettingsCard></>}
        {group === "capture" && <SettingsCard title={t("Fonte de captura")} detail={t("O gravador atual captura a tela inteira do Windows.")}><div className="capture-source-preview" aria-hidden="true"><span>{t("SHMUPX")}</span><span>{t("Captura da tela inteira")}</span></div><dl className="settings-facts"><dt>{t("Origem")}</dt><dd>{t("Tela inteira")}</dd><dt>{t("Início")}</dt><dd>{t("Após autorização da run")}</dd><dt>{t("Encerramento")}</dt><dd>{t("Manual ou pelo limite da ficha")}</dd></dl><p className="muted">{t("A seleção de janela do cliente original ainda não está disponível.")}</p></SettingsCard>}
      </div>
    </div>
  </div>;
}
