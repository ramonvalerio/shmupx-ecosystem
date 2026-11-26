import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { useDialogFocus } from "./useDialogFocus";
import MisterConnect from "./MisterConnect";
import type { GameRoom } from "../types/room";
import { saveSteamLibrary, scanSteam } from "../data/steamLibrary";
import { chooseEmulatorFolder, publish as saveEmulatorLibrary, useEmulatorLibrary } from "../data/emulatorLibrary";
import { usePlatformOwnership } from "../data/platforms";
import type { PlayOn } from "../types/room";
import DialogClose from "./DialogClose";

const SOURCES = [
  { id: "steam", title: "Steam", icon: "S", detail: "Biblioteca da Steam", short: "Importe os jogos instalados na sua Steam.", cta: "Importar agora", body: "Leia os jogos instalados na Steam deste computador para marcar na coleção os que você tem.", available: true },
  { id: "emulator", title: "Emulador", icon: "E", detail: "ROMs do ShmupX Emulator", short: "Importe os ROMs (.zip) deste computador.", cta: "Escolher pasta de ROMs", body: "Leia os ROMs deste computador para marcar na coleção os jogos que você tem. Eles abrem no ShmupX Emulator.", available: true },
  { id: "mister", title: "MiSTer FPGA", icon: "M", detail: "Jogos do MiSTer na rede", short: "Importe os jogos do seu MiSTer pela rede.", cta: "Conectar MiSTer", body: "Informe o IP do MiSTer (Menu › Sistema mostra o endereço). Os jogos de arcade, Mega Drive e Neo Geo dele que têm sala no ShmupX ficam marcados na coleção, e as partidas valem para o ranking.", available: true },
  { id: "local", title: "Jogos locais", icon: "▣", detail: "Jogos instalados no computador", short: "Escolha jogos instalados numa pasta do computador.", cta: "Procurar pasta", body: "Escolha os jogos instalados no computador que deseja incluir na coleção.", available: false },
] as const;

type SourceId = (typeof SOURCES)[number]["id"];

/** Ícones dos cartões de origem (traço simples, na cor do texto). */
function SourceIcon({ id }: { id: SourceId }) {
  const common = { viewBox: "0 0 48 48", width: 52, height: 52, "aria-hidden": true, className: "import-card-icon" } as const;
  switch (id) {
    case "steam": return <svg {...common}><circle cx="24" cy="24" r="20" className="fill" /><circle cx="30.5" cy="18.5" r="6" className="cut" /><circle cx="30.5" cy="18.5" r="3" className="fill" /><circle cx="17" cy="31" r="4.6" className="cut" /><path d="M5 28.5l9.5 4M21 29l7-7.5" className="cut-line" /></svg>;
    case "emulator": return <svg {...common}><path d="M14 14h20c6 0 9.5 4.2 10.4 10.6l1.3 8.9c.6 4.1-2.4 7.5-6.2 7.5-2.3 0-4.3-1.3-5.4-3.3L32 34H16l-2.1 3.7c-1.1 2-3.1 3.3-5.4 3.3-3.8 0-6.8-3.4-6.2-7.5l1.3-8.9C4.5 18.2 8 14 14 14Z" className="fill" /><path d="M14.5 21v8M10.5 25h8" className="cut-line" /><circle cx="32" cy="22.5" r="2.2" className="cut" /><circle cx="36.5" cy="27" r="2.2" className="cut" /></svg>;
    case "mister": return <svg {...common}><path d="M17 5v6M24 5v6M31 5v6M17 37v6M24 37v6M31 37v6M5 17h6M5 24h6M5 31h6M37 17h6M37 24h6M37 31h6" className="line" /><rect x="11" y="11" width="26" height="26" rx="3" className="line" /><text x="24" y="27.5" textAnchor="middle" className="chip-text">FPGA</text></svg>;
    case "local": return <svg {...common}><path d="M4 12a3 3 0 0 1 3-3h11l4 4.5h19a3 3 0 0 1 3 3V37a3 3 0 0 1-3 3H7a3 3 0 0 1-3-3Z" className="line" /><path d="M4 19h40" className="line" /></svg>;
  }
}

export default function ImportGamesDialog({ onClose, onViewCollection, rooms = [] }: { onClose: () => void; onViewCollection?: () => void; rooms?: GameRoom[] }) {
  const { t } = useI18n();
  const [sourceId, setSourceId] = useState<string | null>(null);
  const source = SOURCES.find((candidate) => candidate.id === sourceId);
  const playable = usePlatformOwnership();
  const emulatorLibrary = useEmulatorLibrary().library;
  /** Cartão com ação em andamento (importar, escolher a pasta, atualizar). */
  const [busyId, setBusyId] = useState<string | null>(null);
  /** Motivo, em português, de a última ação de um cartão não ter dado certo. */
  const [cardError, setCardError] = useState<{ id: string; message: string } | null>(null);
  /** Importou alguma biblioteca nesta janela (o tutorial passa ao "Jogos importados!"). */
  const [importedHere, setImportedHere] = useState(false);
  /** Jogos do ShmupX que o jogador tem na plataforma, de quantos rodam nela. */
  function counts(id: PlayOn) {
    const tagged = rooms.map((room) => playable.tags(room).find((tag) => tag.id === id)).filter((tag) => tag !== undefined);
    return { owned: tagged.filter((tag) => tag.owned).length, total: tagged.length };
  }
  /** Roda a ação do cartão, marcando-o como ocupado e guardando o erro, se houver. */
  async function run(id: string, action: () => Promise<void>) {
    setBusyId(id);
    setCardError(null);
    try { await action(); }
    catch (error) { setCardError({ id, message: error instanceof Error ? error.message : String(error) }); }
    finally { setBusyId(null); }
  }
  /** Steam: lê os jogos instalados e já importa, direto no cartão. */
  const importSteamNow = () => run("steam", async () => {
    const result = await scanSteam();
    if (!result.found) throw new Error("Não encontramos a Steam neste computador. Instale a Steam e tente de novo.");
    saveSteamLibrary(result.games.filter((g) => g.installed).map((g) => g.app_id), result.games.length);
    setImportedHere(true);
  });
  /** Emulador: abre a janela do sistema na pasta dos ROMs e importa a pasta escolhida (cancelar não muda nada). */
  const pickRomFolder = () => run("emulator", async () => {
    const result = await chooseEmulatorFolder(t("Pasta de ROMs"));
    if (!result) return;
    if (!result.found) throw new Error("Não encontramos ROMs (arquivos .zip) nessa pasta. Escolha a pasta onde estão os ROMs dos seus jogos.");
    saveEmulatorLibrary({ root: result.root ?? "", roms: result.roms, importedAt: Date.now() });
    setImportedHere(true);
  });
  const refreshLibrary = (id: PlayOn) => run(id, async () => { await playable.library(id).refresh(); });
  function back() { if (sourceId) setSourceId(null); else onClose(); }
  const dialogRef = useDialogFocus(true, back);
  useEffect(() => { dialogRef.current?.querySelector<HTMLButtonElement>("button")?.focus(); }, [sourceId, dialogRef]);

  return <div className="controller-dialog-backdrop">
    <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby="import-dialog-title" tabIndex={-1} className={`controller-dialog import-games-dialog ${source ? "" : "is-picker"}`} data-tutorial="import-dialog" data-tutorial-source={sourceId ?? undefined} data-tutorial-imported={importedHere ? "1" : undefined}>
      <header><div><h2 id="import-dialog-title">{t(source ? "Conectar {platform}" : "Importe os seus jogos", { platform: source ? t(source.title) : "" })}</h2><p>{t(source ? source.detail : "Escolha de onde vêm os seus jogos.")}</p></div><div className="import-header-actions">{source && <button data-dialog-close data-tutorial="import-back" className="secondary-action" onClick={back}>{t("Voltar")}</button>}<DialogClose data-dialog-close={!source ? true : undefined} data-tutorial="import-close" onClick={onClose} /></div></header>
      <div className="controller-dialog-body">
        {!source && <>
          <div className="import-cards" data-tutorial="import-sources">{SOURCES.map((item) => {
            const id = item.id === "local" ? null : (item.id as PlayOn);
            const lib = id ? playable.library(id) : null;
            const connected = Boolean(lib?.connected);
            const count = id && connected ? counts(id) : null;
            const state = !item.available ? "soon" : connected ? "connected" : lib?.off ? "off" : "ready";
            const busy = busyId === item.id;
            const locked = busyId !== null;
            const error = cardError?.id === item.id ? cardError.message : null;
            const actions = id && lib && connected && <div className="import-card-row">
              <button type="button" className="import-card-secondary" disabled={locked} onClick={() => void refreshLibrary(id)} title={t("Lê de novo e importa os jogos instalados depois da última vez")}>{t(busy ? "Atualizando…" : "Atualizar")}</button>
              <button type="button" className="import-card-secondary" disabled={locked} onClick={() => { setCardError(null); lib.disconnect(); }}>{t("Desconectar")}</button>
            </div>;
            return <article key={item.id} className={`import-card is-${state}`}>
              <SourceIcon id={item.id} />
              <h3>{t(item.title)}</h3>
              <p>{t(item.short)}</p>
              <span className="import-card-status">
                <i aria-hidden="true" />
                {state === "soon" ? t("EM BREVE") : state === "connected" ? t("CONECTADO") : state === "off" ? t("DESCONECTADO") : t("PRONTO")}
              </span>
              {count && <small className="import-card-count">{t("{owned} de {total} jogos do ShmupX", { owned: count.owned, total: count.total })}</small>}
              {item.id === "emulator" && connected && emulatorLibrary?.root && <code className="import-card-path" title={emulatorLibrary.root}>{emulatorLibrary.root}</code>}
              {state === "off" && <p className="import-card-off">{t("Você desconectou: o ShmupX não importa sozinho até você importar de novo.")}</p>}
              {error && <p className="import-card-error" role="alert">{t(error)}</p>}
              <div className="import-card-foot">
                {item.id === "steam" && !connected && <button type="button" className="import-card-cta" disabled={locked} onClick={() => void importSteamNow()}>{t(busy ? "Importando…" : item.cta)}</button>}
                {item.id === "emulator" && <button type="button" className="import-card-cta" disabled={locked} onClick={() => void pickRomFolder()}>{t(busy ? "Abrindo…" : connected ? "Alterar pasta" : item.cta)}</button>}
                {item.id === "mister" && <button type="button" className="import-card-cta" disabled={locked} onClick={() => setSourceId(item.id)}>{t(connected ? "Gerenciar" : item.cta)}</button>}
                {item.id === "local" && <button type="button" className="import-card-cta" disabled>{t("Em breve")}</button>}
                {actions}
              </div>
            </article>;
          })}</div>
          <p className="import-cards-note">{t("O ShmupX já importa sozinho a Steam e os ROMs deste computador sempre que abre.")}</p>
        </>}

        {source?.id === "mister" && <>
          <div className="import-platform-heading"><span className="platform-mark" aria-hidden="true">M</span><h3>{t("MiSTer FPGA")}</h3></div>
          <p className="muted">{t(source.body)}</p>
          <MisterConnect rooms={rooms} tutorial onImported={() => setImportedHere(true)} />
        </>}

        {source && !source.available && <><div className="import-platform-heading"><span className="platform-mark" aria-hidden="true">{source.icon}</span><h3>{t(source.title)}</h3></div><p className="muted">{t(source.body)}</p><div className="integration-notice" role="status"><strong>{t("Em breve")}</strong><p>{t("A leitura desta biblioteca ainda não está disponível nesta versão.")}</p></div></>}
      </div>
      {/* Aberta pelas Configurações, leva à coleção; aberta pela coleção, basta o Fechar do topo. */}
      {onViewCollection && <footer style={{ justifyContent: "flex-end" }}><button className="secondary-action" onClick={() => { onClose(); onViewCollection(); }}>{t("Ver coleção")}</button></footer>}
    </div>
  </div>;
}
