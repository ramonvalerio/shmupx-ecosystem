import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { useDialogFocus } from "./useDialogFocus";
import MisterConnect from "./MisterConnect";
import type { GameRoom } from "../types/room";
import { saveSteamLibrary, scanSteam, type SteamGame } from "../data/steamLibrary";
import { publish as saveEmulatorLibrary, romsFor, scanEmulator } from "../data/emulatorLibrary";
import { misterMatches, readMisterLibrary, saveMisterLibrary, scanMister, syncMisterMenu } from "../data/misterLibrary";
import { usePlatformOwnership } from "../data/platforms";
import type { PlayOn } from "../types/room";

const SOURCES = [
  { id: "steam", title: "Steam", icon: "S", detail: "Biblioteca da Steam", body: "Leia os jogos instalados na Steam deste computador para marcar na coleção os que você tem.", available: true },
  { id: "emulator", title: "Emulador", icon: "E", detail: "ROMs do Fightcade (FBNeo)", body: "Leia os ROMs do Fightcade deste computador para marcar na coleção os jogos que você tem.", available: true },
  { id: "mister", title: "MiSTer FPGA", icon: "M", detail: "Jogos do MiSTer na rede", body: "Informe o IP do MiSTer (Menu › Sistema mostra o endereço) para ler os jogos de arcade e os ROMs dele e marcar na coleção os que têm sala no ShmupX.", available: true },
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
  const playable = usePlatformOwnership();
  const [busyId, setBusyId] = useState<string | null>(null);
  const [rowError, setRowError] = useState<string | null>(null);
  /** Jogos do ShmupX que o jogador tem na plataforma, de quantos rodam nela. */
  function counts(id: PlayOn) {
    const tagged = rooms.map((room) => playable.tags(room).find((tag) => tag.id === id)).filter((tag) => tag !== undefined);
    return { owned: tagged.filter((tag) => tag.owned).length, total: tagged.length };
  }
  async function refreshLibrary(id: PlayOn) {
    setBusyId(id);
    setRowError(null);
    try { await playable.library(id).refresh(); }
    catch (error) { setRowError(error instanceof Error ? error.message : String(error)); }
    finally { setBusyId(null); }
  }
  function back() { if (sourceId) { setSourceId(null); setScan({ status: "idle" }); setEmu({ status: "idle" }); setMister((current) => ({ ...current, status: "idle" })); setImported(null); } else onClose(); }
  async function readEmulator() {
    setEmu({ status: "scanning" });
    try {
      const result = await scanEmulator();
      setEmu({ status: "done", found: result.found, root: result.root, roms: result.roms });
    } catch (error) {
      setEmu({ status: "error", error: error instanceof Error ? error.message : String(error) });
    }
  }
  const [mister, setMister] = useState<{ status: "idle" | "scanning" | "done" | "error"; host: string; password: string; mras?: string[]; roms?: string[]; error?: string; menuAdded?: string[]; menuError?: string }>(() => ({ status: "idle", host: readMisterLibrary()?.host ?? "", password: "" }));
  async function readMister() {
    setMister((current) => ({ ...current, status: "scanning", error: undefined }));
    try {
      const result = await scanMister(mister.host, mister.password);
      setMister((current) => ({ ...current, status: "done", host: result.host, mras: result.mras, roms: result.roms, menuAdded: undefined, menuError: undefined }));
    } catch (error) {
      setMister((current) => ({ ...current, status: "error", error: error instanceof Error ? error.message : String(error) }));
    }
  }
  const misterRooms = mister.status === "done" ? rooms.filter((room) => misterMatches(room, { mras: mister.mras ?? [], roms: mister.roms ?? [] })) : [];
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
        {!source && <div className="import-source-list">{SOURCES.map((item) => {
          const id = item.id === "local" ? null : (item.id as PlayOn);
          const lib = id ? playable.library(id) : null;
          const count = id && lib?.connected ? counts(id) : null;
          return <div key={item.id} className={`import-source-row ${lib?.connected ? "is-connected" : ""}`}>
            <button className="import-source-option" onClick={() => setSourceId(item.id)}>
              <span className="platform-mark" aria-hidden="true">{item.icon}</span>
              <span><strong>{t(item.title)}</strong><small>{count ? t("✓ Conectada · {owned} de {total} jogos do ShmupX", { owned: count.owned, total: count.total }) : t(item.detail)}</small></span>
              {!lib?.connected && <span className={`import-availability ${item.available ? "is-available" : ""}`}>{t(item.available ? "Disponível" : "Em breve")}</span>}
            </button>
            {id && lib?.connected && <div className="import-source-actions">
              <button type="button" className="secondary-action" disabled={busyId !== null} onClick={() => void refreshLibrary(id)} title={t("Lê de novo e importa os jogos instalados depois da última vez")}>{t(busyId === id ? "Atualizando…" : "Atualizar")}</button>
              <button type="button" className="secondary-action" disabled={busyId !== null} onClick={lib.disconnect}>{t("Desconectar")}</button>
            </div>}
          </div>;
        })}{rowError && <p className="mode-error" role="alert">{t(rowError)}</p>}</div>}

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

        {source?.id === "mister" && <>
          <div className="import-platform-heading"><span className="platform-mark" aria-hidden="true">M</span><h3>{t("MiSTer FPGA")}</h3></div>
          <p className="muted">{t(source.body)}</p>
          <form className="mister-connect-form" onSubmit={(event) => { event.preventDefault(); void readMister(); }}>
            <label className="settings-field">{t("IP do MiSTer")}<input value={mister.host} onChange={(event) => setMister((current) => ({ ...current, host: event.target.value }))} placeholder="192.168.0.23" inputMode="decimal" autoComplete="off" required /></label>
            <label className="settings-field">{t("Senha do FTP (vazio = padrão)")}<input type="password" value={mister.password} onChange={(event) => setMister((current) => ({ ...current, password: event.target.value }))} autoComplete="off" /></label>
            <button type="submit" className="primary-action" disabled={mister.status === "scanning" || !mister.host.trim()}>{t(mister.status === "scanning" ? "Lendo os jogos do MiSTer…" : "Ler jogos do MiSTer")}</button>
          </form>
          {mister.status === "error" && <p className="mode-error" role="alert">{t(mister.error ?? "Não foi possível ler o MiSTer.")}</p>}
          {mister.status === "done" && <>
            <p role="status"><strong>{t("{mras} jogos de arcade e {roms} ROMs no MiSTer {host}.", { mras: (mister.mras ?? []).length, roms: (mister.roms ?? []).length, host: mister.host })}</strong></p>
            {misterRooms.length > 0
              ? <><p className="muted">{t("Com sala no ShmupX:")}</p><ul className="import-found-list">{misterRooms.map((room) => <li key={room.id}><span className="supported">✓</span> {room.title}</li>)}</ul></>
              : <p className="muted">{t("Nenhum jogo do MiSTer tem sala no ShmupX ainda.")}</p>}
            {imported === null
              ? <button className="primary-action" onClick={() => {
                  saveMisterLibrary({ host: mister.host, mras: mister.mras ?? [], roms: mister.roms ?? [], importedAt: Date.now() });
                  setImported(misterRooms.length);
                  // Os jogos do Lab marcados com MiSTer que ele tem entram no menu ShmupX do MiSTer.
                  syncMisterMenu(mister.host, mister.password, rooms, { mras: mister.mras ?? [] })
                    .then((added) => setMister((current) => ({ ...current, menuAdded: added })))
                    .catch((error) => setMister((current) => ({ ...current, menuError: error instanceof Error ? error.message : String(error) })));
                }}>{t("Importar biblioteca")}</button>
              : <p className="success-text" role="status">{t("Biblioteca importada. A coleção agora mostra os jogos que você tem.")}</p>}
            {mister.menuAdded && <p className="muted" role="status">{mister.menuAdded.length > 0
              ? t("Menu ShmupX do MiSTer atualizado: {games}.", { games: mister.menuAdded.join(", ") })
              : t("O menu ShmupX do MiSTer já tem todos os jogos do ShmupX que ele tem.")}</p>}
            {mister.menuError && <p className="mode-error" role="alert">{t("Não foi possível atualizar o menu ShmupX do MiSTer: {reason}", { reason: mister.menuError })}</p>}
          </>}
          <div className="import-platform-heading"><h3>{t("Partidas do MiSTer no ranking")}</h3></div>
          <p className="muted">{t("Jogue no MiSTer e as partidas entram no seu perfil e no ranking, como as do PC. Receba no seu e-mail o token e cole a linha no shmupx.ini do MiSTer.")}</p>
          <MisterConnect host={mister.host || undefined} password={mister.password} />
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
