import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { useDialogFocus } from "./useDialogFocus";
import type { GameRoom } from "../types/room";
import { saveSteamLibrary, scanSteam, type SteamGame } from "../data/steamLibrary";
import { publish as saveEmulatorLibrary, romsFor, scanEmulator } from "../data/emulatorLibrary";

const SOURCES = [
  { id: "steam", title: "Steam", icon: "S", detail: "Biblioteca da Steam", body: "Leia os jogos instalados na Steam deste computador para marcar na coleção os que você tem.", available: true },
  { id: "emulator", title: "Emulador", icon: "E", detail: "ROMs do Fightcade (FBNeo)", body: "Leia os ROMs do Fightcade deste computador para marcar na coleção os jogos que você tem.", available: true },
  { id: "local", title: "Jogos locais", icon: "▣", detail: "Jogos instalados no computador", body: "Escolha os jogos instalados no computador que deseja incluir na coleção.", available: false },
] as const;

type Scan = { status: "idle" | "scanning" | "done" | "error"; found?: boolean; games?: SteamGame[]; total?: number; error?: string };

export default function ImportGamesDialog({ onClose, onViewCollection, rooms = [] }: { onClose: () => void; onViewCollection?: () => void; rooms?: GameRoom[] }) {
  const { t } = useI18n();
  const [sourceId, setSourceId] = useState<string | null>(null);
  const [scan, setScan] = useState<Scan>({ status: "idle" });
  const [imported, setImported] = useState<number | null>(null);
  const [emu, setEmu] = useState<{ status: "idle" | "scanning" | "done" | "error"; found?: boolean; root?: string | null; roms?: string[]; error?: string }>({ status: "idle" });
  const source = SOURCES.find((candidate) => candidate.id === sourceId);
  function back() { if (sourceId) { setSourceId(null); setScan({ status: "idle" }); setEmu({ status: "idle" }); setImported(null); } else onClose(); }
  async function readEmulator() {
    setEmu({ status: "scanning" });
    try {
      const result = await scanEmulator();
      setEmu({ status: "done", found: result.found, root: result.root, roms: result.roms });
    } catch (error) {
      setEmu({ status: "error", error: error instanceof Error ? error.message : String(error) });
    }
  }
  const emuRoms = emu.roms ?? [];
  const emuRooms = rooms.filter((room) => romsFor(room).some((rom) => emuRoms.includes(rom)));
  const dialogRef = useDialogFocus(true, back);
  useEffect(() => { dialogRef.current?.querySelector<HTMLButtonElement>("button")?.focus(); }, [sourceId, scan.status, dialogRef]);

  async function readSteam() {
    setScan({ status: "scanning" });
    try {
      const result = await scanSteam();
      setScan({ status: "done", found: result.found, games: result.games.filter((g) => g.installed), total: result.games.length });
    } catch (error) {
      setScan({ status: "error", error: error instanceof Error ? error.message : String(error) });
    }
  }
  function importSteam() {
    const games = scan.games ?? [];
    saveSteamLibrary(games.map((g) => g.app_id), scan.total ?? games.length);
    setImported(games.length);
  }

  const installed = scan.games ?? [];
  const withRoom = rooms.filter((room) => room.steamAppId > 0 && installed.some((g) => g.app_id === room.steamAppId));
  const withoutRoom = installed.length - withRoom.length;

  return <div className="controller-dialog-backdrop">
    <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby="import-dialog-title" tabIndex={-1} className="controller-dialog import-games-dialog">
      <header><div><h2 id="import-dialog-title">{t(source ? "Conectar {platform}" : "Importar jogos", { platform: source ? t(source.title) : "" })}</h2><p>{t(source ? source.detail : "Escolha de onde deseja trazer seus jogos.")}</p></div><div className="import-header-actions">{source && <button data-dialog-close className="secondary-action" onClick={back}>{t("Voltar")}</button>}<button data-dialog-close={!source ? true : undefined} className="secondary-action" onClick={onClose}>{t("Fechar")}</button></div></header>
      <div className="controller-dialog-body">
        {!source && <div className="import-source-list">{SOURCES.map((item) => <button key={item.id} className="import-source-option" onClick={() => setSourceId(item.id)}><span className="platform-mark" aria-hidden="true">{item.icon}</span><span><strong>{t(item.title)}</strong><small>{t(item.detail)}</small></span><span className={`import-availability ${item.available ? "is-available" : ""}`}>{t(item.available ? "Disponível" : "Em breve")}</span></button>)}</div>}

        {source?.id === "emulator" && <>
          <div className="import-platform-heading"><span className="platform-mark" aria-hidden="true">E</span><h3>{t("Emulador")}</h3></div>
          <p className="muted">{t(source.body)}</p>
          {emu.status === "idle" && <button className="primary-action" onClick={() => void readEmulator()}>{t("Ler ROMs do Fightcade")}</button>}
          {emu.status === "scanning" && <p className="muted" role="status">{t("Lendo os ROMs…")}</p>}
          {emu.status === "error" && <p className="mode-error" role="alert">{emu.error}</p>}
          {emu.status === "done" && !emu.found && <div className="integration-notice" role="alert"><strong>{t("Fightcade não encontrado")}</strong><p>{t("Não encontramos o Fightcade neste computador. Instale o Fightcade e tente de novo.")}</p></div>}
          {emu.status === "done" && emu.found && <>
            <p role="status"><strong>{t("{count} ROMs em {folder}.", { count: emuRoms.length, folder: emu.root ?? "" })}</strong></p>
            {emuRooms.length > 0
              ? <><p className="muted">{t("Com sala no ShmupX:")}</p><ul className="import-found-list">{emuRooms.map((room) => <li key={room.id}><span className="supported">✓</span> {room.title}</li>)}</ul></>
              : <p className="muted">{t("Nenhum dos seus ROMs tem sala no ShmupX ainda.")}</p>}
            {imported === null
              ? <button className="primary-action" onClick={() => { saveEmulatorLibrary({ root: emu.root ?? "", roms: emuRoms, importedAt: Date.now() }); setImported(emuRoms.length); }}>{t("Importar biblioteca")}</button>
              : <p className="success-text" role="status">{t("Biblioteca importada. A coleção agora mostra os jogos que você tem.")}</p>}
          </>}
        </>}

        {source && !source.available && <><div className="import-platform-heading"><span className="platform-mark" aria-hidden="true">{source.icon}</span><h3>{t(source.title)}</h3></div><p className="muted">{t(source.body)}</p><div className="integration-notice" role="status"><strong>{t("Em breve")}</strong><p>{t("A leitura desta biblioteca ainda não está disponível nesta versão.")}</p></div></>}

        {source?.id === "steam" && <>
          <div className="import-platform-heading"><span className="platform-mark" aria-hidden="true">S</span><h3>{t("Steam")}</h3></div>
          <p className="muted">{t(source.body)}</p>
          {scan.status === "idle" && <button className="primary-action" onClick={() => void readSteam()}>{t("Ler biblioteca da Steam")}</button>}
          {scan.status === "scanning" && <p className="muted" role="status">{t("Lendo os jogos instalados…")}</p>}
          {scan.status === "error" && <p className="mode-error" role="alert">{scan.error}</p>}
          {scan.status === "done" && !scan.found && <div className="integration-notice" role="alert"><strong>{t("Steam não encontrada")}</strong><p>{t("Não encontramos a Steam neste computador. Instale a Steam e tente de novo.")}</p></div>}
          {scan.status === "done" && scan.found && <>
            <p role="status"><strong>{t("{count} jogos instalados na sua Steam.", { count: installed.length })}</strong></p>
            {withRoom.length > 0
              ? <><p className="muted">{t("Com sala no ShmupX:")}</p><ul className="import-found-list">{withRoom.map((room) => <li key={room.id}><span className="supported">✓</span> {room.title}</li>)}</ul></>
              : <p className="muted">{t("Nenhum dos jogos instalados tem sala no ShmupX ainda.")}</p>}
            {withoutRoom > 0 && <p className="muted">{t("{count} outros jogos ainda não têm sala no ShmupX.", { count: withoutRoom })}</p>}
            {imported === null
              ? <button className="primary-action" onClick={importSteam}>{t("Importar biblioteca")}</button>
              : <p className="success-text" role="status">{t("Biblioteca importada. A coleção agora mostra os jogos que você tem.")}</p>}
          </>}
        </>}
      </div>
      <footer><span className="muted">{t("Os dados são lidos só neste computador.")}</span>{onViewCollection && <button className="secondary-action" onClick={() => { onClose(); onViewCollection(); }}>{t("Ver coleção")}</button>}{!onViewCollection && <button className="secondary-action" onClick={onClose}>{t("Voltar à coleção")}</button>}</footer>
    </div>
  </div>;
}
