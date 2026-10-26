import { useI18n } from "../i18n";
import { useState, type ReactNode } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { appDataDir } from "@tauri-apps/api/path";
import { PageHeading, type Section } from "./ClientPages";
import { defaultPreferences, type ClientPreferences } from "./settingsState";
import ConfigureControllerDialog from "./ConfigureControllerDialog";
import { readMappings } from "./controllerMappings";
import ImportGamesDialog from "./ImportGamesDialog";

const GROUPS = [
  { id: "connections", title: "Importar jogos", detail: "Bibliotecas e plataformas", icon: "▦" },
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

export default function SettingsPage({ preferences, onPreferences, storageError, onNavigate }: {
  preferences: ClientPreferences; onPreferences: (value: ClientPreferences) => void; storageError: boolean; onNavigate: (section: Section) => void;
}) {
  const { t } = useI18n();
  const [group, setGroup] = useState<typeof GROUPS[number]["id"]>("interface");
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
    {importOpen && <ImportGamesDialog onClose={()=>setImportOpen(false)} onViewCollection={()=>onNavigate("collection")} />}
    {configureController && <ConfigureControllerDialog onClose={()=>{setConfigureController(false);detectControllers();}} />}
    <PageHeading title={t("Configurações")} detail={t("Biblioteca, captura e preferências do cliente.")} />
    <div className="settings-layout">
      <nav className="settings-menu" aria-label={t("Grupos de configurações")}>{GROUPS.map((item) => <button key={item.id} aria-current={group === item.id ? "page" : undefined} onClick={() => setGroup(item.id)}><span aria-hidden="true">{item.icon}</span><span><strong>{t(item.title)}</strong><small>{t(item.detail)}</small></span></button>)}</nav>
      <div className="settings-panels">
        {group === "interface" && <>
          <SettingsCard title={t("Idioma do sistema")} detail={t("O idioma muda na hora e fica salvo neste dispositivo.")}><label className="settings-field">{t("Idioma do sistema")}<select value={preferences.language} onChange={(event) => onPreferences({ ...preferences, language: event.target.value === "en" ? "en" : "pt-BR" })}><option value="pt-BR">{t("Português (Brasil)")}</option><option value="en">{t("English")}</option></select></label></SettingsCard>
          <SettingsCard title={t("Visual")} detail={t("Escolha a paleta do ShmupX. A mudança vale na hora e fica guardada para a próxima vez.")}>
            <div className="theme-options" role="group" aria-label={t("Visual")}>{([{ id: "default", label: "Padrão" }, { id: "dark", label: "Escuro" }, { id: "light", label: "Claro" }] as const).map((theme) => <button key={theme.id} aria-pressed={preferences.appearance === theme.id} onClick={() => onPreferences({ ...preferences, appearance: theme.id })}><span className={`theme-swatch swatch-${theme.id}`} aria-hidden="true" />{t(theme.label)}</button>)}</div>
            <label className="checkbox-field"><input type="checkbox" checked={preferences.compact} onChange={(event) => onPreferences({ ...preferences, compact: event.target.checked })} />{t("Coleção compacta")}</label>
          </SettingsCard>
          <SettingsCard title={t("Abrir ao iniciar")} detail={t("Escolha a tela e o modo de apresentação para a próxima abertura do cliente.")}>
            <label className="settings-field">{t("Tela inicial")}<select value={preferences.startup} onChange={(event) => onPreferences({ ...preferences, startup: event.target.value as ClientPreferences["startup"] })}><option value="home">{t("Início")}</option><option value="collection">{t("Coleção")}</option><option value="ranking">{t("Ranking")}</option><option value="settings">{t("Configurações")}</option></select></label>
            <label className="checkbox-field"><input type="checkbox" checked={preferences.startupBigPicture} onChange={(event) => onPreferences({ ...preferences, startupBigPicture: event.target.checked })} />{t("Abrir em modo Big Picture")}</label>
            <p className="muted">{t("No desktop, Big Picture usa tela cheia. Na prévia do navegador, altera a apresentação.")}</p>
          </SettingsCard>
          <SettingsCard title={t("Preferências da interface")}><p className="muted" role="status">{storageError ? t("Preferências aplicadas nesta sessão. Não foi possível salvar neste dispositivo.") : t("As alterações são aplicadas e salvas automaticamente neste dispositivo.")}</p><button className="secondary-action" onClick={() => onPreferences(defaultPreferences())}>{t("Restaurar preferências da interface")}</button></SettingsCard>
        </>}
        {group === "connections" && <SettingsCard title={t("Conexões de biblioteca")} detail={t("Reúna seus jogos no cliente.")}><div className="platform-connection"><span className="platform-mark">{t("S")}</span><div><h3>{t("Steam")}</h3><span className="sample-chip">{t("CATÁLOGO DE DEMONSTRAÇÃO")}</span><p className="muted">{t("O lançamento de jogos pela Steam está disponível no aplicativo desktop. A biblioteca instalada ainda não é sincronizada.")}</p><button className="primary-action" onClick={()=>setImportOpen(true)}>{t("Importar jogos")}</button><button className="text-action" onClick={() => onNavigate("collection")}>{t("Ver coleção")}</button></div></div><p className="muted">{t("Importação de jogos locais e de emuladores ainda não disponível nesta versão.")}</p></SettingsCard>}
        {group === "controller" && <><SettingsCard title={t("Dispositivo de entrada")} detail={t("Consulte os controles que este ambiente disponibiliza ao cliente.")}><button className="secondary-action" onClick={detectControllers}>{t("Detectar controles")}</button><p className="muted" role="status">{controllerMessage === "controllers-detected" ? t("{count} controle(s) detectado(s).",{count:controllers.length}) : t(controllerMessage)}</p>{controllers.length > 0 && <ul className="controller-devices">{controllers.map((device, index) => <li key={`${index}:${device.id}`}>{device.id} · {t(device.standard ? "compatível com Big Picture" : "mapeamento não padrão")}</li>)}</ul>}</SettingsCard><SettingsCard title={t("Comandos do Big Picture")}><dl className="settings-facts"><dt>{t("Navegar")}</dt><dd>{t("Direcional ou analógico esquerdo")}</dd><dt>{t("Confirmar")}</dt><dd>{t("A")}</dd><dt>{t("Voltar / fechar")}</dt><dd>{t("B")}</dd><dt>{t("Teclado")}</dt><dd>{t("Setas, Enter e Esc")}</dd><dt>{t("Alternar Big Picture")}</dt><dd>{t("Alt+Enter")}</dd><dt>{t("Comunicação por voz no desktop")}</dt><dd>{t("Select / View / Back")}</dd></dl><button className="primary-action" onClick={()=>setConfigureController(true)}>{t("Configurar controle")}</button><p className="muted">{t("O perfil personalizado afeta apenas a navegação do Big Picture.")}</p></SettingsCard></>}
        {group === "storage" && <SettingsCard title={t("Destino das gravações")} detail={t("As runs são salvas na pasta de dados do aplicativo.")}><button className="secondary-action" disabled={folderBusy} onClick={locateFolder}>{folderBusy ? t("Consultando…") : t("Consultar pasta atual")}</button>{folder && <p className="folder-result" role="status">{t(folder)}</p>}<p className="muted">{t("A seleção de outra pasta e a listagem de replays ainda não estão disponíveis.")}</p><button className="text-action" onClick={() => onNavigate("submissions")}>{t("Ver runs desta sessão →")}</button></SettingsCard>}
        {group === "recording" && <><SettingsCard title={t("Vídeo")} detail={t("Parâmetros atuais do gravador, somente para consulta.")}><dl className="settings-facts"><dt>{t("Taxa de quadros")}</dt><dd>{t("60 fps")}</dd><dt>{t("Codificador")}</dt><dd>{t("H.264")}</dd><dt>{t("Preset")}</dt><dd>{t("Ultrafast")}</dd><dt>{t("Formato")}</dt><dd>{t("MP4")}</dd></dl></SettingsCard><SettingsCard title={t("Áudio")}><dl className="settings-facts"><dt>{t("Fonte")}</dt><dd>{t("Áudio do sistema")}</dd><dt>{t("Captura")}</dt><dd>{t("Dispositivo de loopback")}</dd></dl><p className="muted">{t("A seleção do dispositivo e a edição dos parâmetros ainda não estão disponíveis.")}</p></SettingsCard></>}
        {group === "capture" && <SettingsCard title={t("Fonte de captura")} detail={t("O gravador atual captura a tela inteira do Windows.")}><div className="capture-source-preview" aria-hidden="true"><span>{t("SHMUPX")}</span><span>{t("Captura da tela inteira")}</span></div><dl className="settings-facts"><dt>{t("Origem")}</dt><dd>{t("Tela inteira")}</dd><dt>{t("Início")}</dt><dd>{t("Após autorização da run")}</dd><dt>{t("Encerramento")}</dt><dd>{t("Manual ou pelo limite da ficha")}</dd></dl><p className="muted">{t("A seleção de janela do cliente original ainda não está disponível.")}</p></SettingsCard>}
      </div>
    </div>
  </div>;
}
