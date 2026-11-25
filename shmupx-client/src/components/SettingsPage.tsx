import { useI18n } from "../i18n";
import { useState, type ReactNode } from "react";
import { PageHeading, type Section } from "./ClientPages";
import { defaultPreferences, type ClientPreferences } from "./settingsState";
import ImportGamesDialog from "./ImportGamesDialog";
import CabinetControlMapper from "./CabinetControlMapper";
import MisterConnect from "./MisterConnect";
import ProfileSettings from "./ProfileSettings";
import StorageSettings from "./StorageSettings";
import type { PlayerProfile } from "../auth/firebase";
import { useYouTube } from "../data/useLive";
import YouTubeNoChannelDialog from "./YouTubeNoChannelDialog";
import { clearSteamLibrary, useSteamLibrary } from "../data/steamLibrary";
import { publish as saveEmulatorLibrary, romsFor, useEmulatorLibrary } from "../data/emulatorLibrary";
import type { GameRoom } from "../types/room";
import { EMULATOR_RESOLUTIONS, defaultEmulatorSettings, useEmulatorSettings, type EmulatorResolution } from "../data/emulatorSettings";

const GROUPS = [
  { id: "profile", title: "Perfil", detail: "Avatar, nome, nick e assinatura", icon: "◉" },
  { id: "connections", title: "Importar jogos", detail: "Bibliotecas, plataformas e MiSTer FPGA", icon: "▦" },
  { id: "integrations", title: "Integrações", detail: "YouTube", icon: "▷" },
  { id: "emulator", title: "Emulador", detail: "Janela, scanlines e resolução", icon: "▶" },
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

/** Como o ShmupX Emulator abre os jogos de arcade neste PC. Vale para a próxima vez que o jogo abrir. */
function EmulatorSettingsCard() {
  const { t } = useI18n();
  const { settings, update, saved } = useEmulatorSettings();
  const crt = settings.resolution === "crt15";
  const resolution = EMULATOR_RESOLUTIONS.find((r) => r.id === settings.resolution) ?? EMULATOR_RESOLUTIONS[0];
  return <>
    <SettingsCard title="Vídeo do emulador" detail="Como o ShmupX Emulator abre os jogos de arcade neste computador. Vale a partir do próximo jogo aberto.">
      <label className="checkbox-field"><input type="checkbox" checked={settings.windowed} onChange={(event) => update({ ...settings, windowed: event.target.checked })} />{t("Abrir em janela")}</label>
      <label className="checkbox-field"><input type="checkbox" checked={settings.scanlines && !crt} disabled={crt} onChange={(event) => update({ ...settings, scanlines: event.target.checked })} />{t("Scanlines (linhas de TV de tubo)")}</label>
      <label className="settings-field">{t("Resolução de vídeo")}<select value={settings.resolution} onChange={(event) => update({ ...settings, resolution: event.target.value as EmulatorResolution })}>{EMULATOR_RESOLUTIONS.map((r) => <option key={r.id} value={r.id}>{t(r.label)}</option>)}</select></label>
      <p className="muted">{t(resolution.detail)}</p>
    </SettingsCard>
    <SettingsCard title="Preferências do emulador"><p className="muted" role="status">{saved ? t("As alterações são salvas automaticamente neste computador.") : t("Preferências aplicadas nesta sessão. Não foi possível salvar neste dispositivo.")}</p><button className="secondary-action" onClick={() => update(defaultEmulatorSettings())}>{t("Restaurar padrão do emulador")}</button></SettingsCard>
  </>;
}

/** YouTube do jogador: conectar o canal para transmitir ao vivo (START LIVE na sala). */
function YouTubeCard() {
  const { t } = useI18n();
  const { connection, busy, error, noChannel, dismissNoChannel, connect, disconnect } = useYouTube();
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
    {noChannel && <YouTubeNoChannelDialog busy={busy} onRetry={() => void connect()} onClose={dismissNoChannel} />}
    <p className="muted">{t("O ShmupX pede permissão para criar e encerrar transmissões no seu canal. As lives são não listadas, a menos que você marque \"Live pública\" na sala.")}</p>
  </SettingsCard>;
}

/** Uma biblioteca conectada (Steam, ROMs do emulador, Local): situação, contagem e ações. */
function LibraryRow({ icon, name, connected, soon, games, installed, installedLabel, syncedAt, folder, busy, onConnect, onSync, onDisconnect, onChooseFolder }: {
  icon: ReactNode; name: string; connected: boolean; soon?: boolean; games?: number; installed?: number; installedLabel?: string; syncedAt?: number; folder?: string; busy?: boolean;
  onConnect?: () => void; onSync?: () => void; onDisconnect?: () => void;
  /** Escolher a pasta da biblioteca à mão (ex.: ROMs fora das pastas de costume). */
  onChooseFolder?: () => void;
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
          ? <><button className="secondary-action" disabled={busy} onClick={onSync}>{t(busy ? "Sincronizando…" : "Sincronizar")}</button>{onChooseFolder && <button className="secondary-action" disabled={busy} onClick={onChooseFolder}>{t("Alterar pasta")}</button>}<button className="secondary-action" disabled={busy} onClick={onDisconnect}>{t("Desconectar")}</button></>
          : <><button className="secondary-action" disabled={soon || busy} onClick={onConnect} title={soon ? t("A leitura desta biblioteca ainda não está disponível nesta versão.") : undefined}>{t(busy ? "Conectando…" : "Conectar")}</button>{onChooseFolder && <button className="secondary-action" disabled={busy} onClick={onChooseFolder}>{t("Selecionar pasta")}</button>}</>}
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
      if (!result.found) setError(t("Não encontramos ROMs neste computador. Se eles estão em outra pasta, clique em “Selecionar pasta” e indique onde."));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setEmuBusy(false);
    }
  }
  /** Pasta de ROMs escolhida à mão: guarda no app e conecta (ou atualiza) a biblioteca. */
  async function chooseEmulatorFolder() {
    setEmuBusy(true);
    setError(null);
    try {
      const result = await emulator.chooseFolder(t("Pasta de ROMs"));
      if (result?.found) saveEmulatorLibrary({ root: result.root ?? "", roms: result.roms, importedAt: Date.now() });
    } catch (e) {
      setError(t(e instanceof Error ? e.message : String(e)));
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
        onConnect={() => void syncEmulator()} onSync={() => void syncEmulator()} onDisconnect={emulator.disconnect} onChooseFolder={() => void chooseEmulatorFolder()} />
      <LibraryRow icon={<IconFolder />} name={t("Local")} connected={false} soon />
    </div>
    {error && <p className="mode-error" role="alert">{error}</p>}
    <button className="primary-action library-import" onClick={onImport}><span aria-hidden="true">+</span> {t("Importar jogos")}</button>
  </SettingsCard>;
}

export default function SettingsPage({ rooms = [], preferences, onPreferences, storageError, onNavigate, profile, onProfile, onReplayTutorial }: {
  rooms?: GameRoom[]; preferences: ClientPreferences; onPreferences: (value: ClientPreferences) => void; storageError: boolean; onNavigate: (section: Section) => void;
  profile: PlayerProfile; onProfile: (profile: PlayerProfile) => void;
  /** "Rever tutorial": fecha as Configurações e abre o passo a passo de importar e jogar. */
  onReplayTutorial?: () => void;
}) {
  const { t } = useI18n();
  const [group, setGroup] = useState<typeof GROUPS[number]["id"]>("profile");
  const [importOpen,setImportOpen] = useState(false);

  return <div className="client-page settings-page">
    {importOpen && <ImportGamesDialog rooms={rooms} onClose={()=>setImportOpen(false)} onViewCollection={()=>onNavigate("collection")} />}
    <PageHeading title={t("Configurações")} detail={t("Biblioteca, captura e preferências do cliente.")} />
    <div className="settings-layout">
      <nav className="settings-menu" aria-label={t("Grupos de configurações")}>{GROUPS.map((item) => <button key={item.id} aria-current={group === item.id ? "page" : undefined} onClick={() => setGroup(item.id)}><span aria-hidden="true">{item.icon}</span><span><strong>{t(item.title)}</strong><small>{t(item.detail)}</small></span></button>)}</nav>
      <div className="settings-panels">
        {group === "profile" && <SettingsCard title={t("Perfil")}><ProfileSettings profile={profile} onProfile={onProfile} /></SettingsCard>}
        {group === "interface" && <>
          <SettingsCard title={t("Idioma do sistema")} detail={t("O idioma muda na hora e fica salvo neste dispositivo.")}><label className="settings-field">{t("Idioma do sistema")}<select value={preferences.language} onChange={(event) => onPreferences({ ...preferences, language: event.target.value === "en" ? "en" : "pt-BR" })}><option value="pt-BR">{t("Português (Brasil)")}</option><option value="en">{t("English")}</option></select></label></SettingsCard>
          <SettingsCard title={t("Visual")} detail={t("Escolha a paleta do ShmupX. A mudança vale na hora e fica guardada para a próxima vez.")}>
            <div className="theme-options" role="group" aria-label={t("Visual")}>{([{ id: "default", label: "Padrão" }, { id: "dark", label: "Escuro" }, { id: "light", label: "Claro" }, { id: "fashion", label: "Fashion" }] as const).map((theme) => <button key={theme.id} aria-pressed={preferences.appearance === theme.id} onClick={() => onPreferences({ ...preferences, appearance: theme.id })}><span className={`theme-swatch swatch-${theme.id}`} aria-hidden="true" />{t(theme.label)}</button>)}</div>
            <label className="checkbox-field"><input type="checkbox" checked={preferences.compact} onChange={(event) => onPreferences({ ...preferences, compact: event.target.checked })} />{t("Coleção compacta")}</label>
          </SettingsCard>
          <SettingsCard title={t("Abrir ao iniciar")} detail={t("O cliente sempre abre na Coleção. Escolha o modo de apresentação.")}>
            <label className="checkbox-field"><input type="checkbox" checked={preferences.startupBigPicture} onChange={(event) => onPreferences({ ...preferences, startupBigPicture: event.target.checked })} />{t("Abrir em modo Big Picture")}</label>
            <p className="muted">{t("No desktop, Big Picture usa tela cheia. Na prévia do navegador, altera a apresentação.")}</p>
          </SettingsCard>
          {onReplayTutorial && <SettingsCard title={t("Tutorial")} detail={t("Mostra de novo o passo a passo para importar os seus jogos e começar a jogar.")}><button className="secondary-action" onClick={onReplayTutorial}>{t("Rever tutorial")}</button></SettingsCard>}
          <SettingsCard title={t("Preferências da interface")}><p className="muted" role="status">{storageError ? t("Preferências aplicadas nesta sessão. Não foi possível salvar neste dispositivo.") : t("As alterações são aplicadas e salvas automaticamente neste dispositivo.")}</p><button className="secondary-action" onClick={() => onPreferences(defaultPreferences())}>{t("Restaurar preferências da interface")}</button></SettingsCard>
        </>}
        {group === "connections" && <><ConnectedLibraries rooms={rooms} onImport={() => setImportOpen(true)} /><SettingsCard title={t("MiSTer FPGA")} detail={t("Jogue no MiSTer e as partidas entram no seu perfil e no ranking, como as do PC.")}><MisterConnect /></SettingsCard></>}
        {group === "integrations" && <YouTubeCard />}
        {group === "emulator" && <EmulatorSettingsCard />}
        {group === "controller" && <><SettingsCard title="Controles do emulador" detail="Clique num botão do gabinete e aperte o botão do seu controle ou uma tecla. Vale para todos os jogos no ShmupX Emulator."><CabinetControlMapper /></SettingsCard></>}
        {group === "storage" && <SettingsCard title={t("Destino das gravações")} detail={t("Pasta onde os vídeos das runs são gravados.")}><StorageSettings /></SettingsCard>}
        {group === "recording" && <><SettingsCard title={t("Vídeo")} detail={t("Parâmetros atuais do gravador, somente para consulta.")}><dl className="settings-facts"><dt>{t("Taxa de quadros")}</dt><dd>{t("60 fps")}</dd><dt>{t("Codificador")}</dt><dd>{t("H.264")}</dd><dt>{t("Preset")}</dt><dd>{t("Ultrafast")}</dd><dt>{t("Formato")}</dt><dd>{t("MP4")}</dd></dl></SettingsCard><SettingsCard title={t("Áudio")}><dl className="settings-facts"><dt>{t("Fonte")}</dt><dd>{t("Áudio do sistema")}</dd><dt>{t("Captura")}</dt><dd>{t("Dispositivo de loopback")}</dd></dl><p className="muted">{t("A seleção do dispositivo e a edição dos parâmetros ainda não estão disponíveis.")}</p></SettingsCard></>}
        {group === "capture" && <SettingsCard title={t("Fonte de captura")} detail={t("O gravador atual captura a tela inteira do Windows.")}><div className="capture-source-preview" aria-hidden="true"><span>{t("SHMUPX")}</span><span>{t("Captura da tela inteira")}</span></div><dl className="settings-facts"><dt>{t("Origem")}</dt><dd>{t("Tela inteira")}</dd><dt>{t("Início")}</dt><dd>{t("Após autorização da run")}</dd><dt>{t("Encerramento")}</dt><dd>{t("Manual ou pelo limite da ficha")}</dd></dl><p className="muted">{t("A seleção de janela do cliente original ainda não está disponível.")}</p></SettingsCard>}
      </div>
    </div>
  </div>;
}
