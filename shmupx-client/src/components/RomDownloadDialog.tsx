import { useI18n } from "../i18n";
import type { GameRoom } from "../types/room";
import { closeRomDownload, romDownloadPercent, useRomDownload } from "../data/romDownload";
import { useDialogFocus } from "./useDialogFocus";
import DialogClose from "./DialogClose";

const megabytes = (bytes: number) => (bytes / (1024 * 1024)).toFixed(1);

/**
 * Modal do download dos ROMs do `roms.json` (aberta ao escolher a pasta de ROMs): barra de
 * progresso com o jogo e o arquivo atual e, no fim, o resumo. Só fecha depois de terminar.
 */
export function RomDownloadDialog({ rooms }: { rooms: GameRoom[] }) {
  const { t } = useI18n();
  const v = useRomDownload();
  const finished = v.stage === "done";
  const ref = useDialogFocus(v.open, () => { if (finished) closeRomDownload(); });
  if (!v.open) return null;
  const percent = romDownloadPercent(v);
  const title = (id: string) => rooms.find((room) => room.id === id)?.title ?? id;
  const status = v.stage === "reading"
    ? t("Lendo a lista de ROMs…")
    : finished
      ? v.noCatalog
        ? t("Entre na sua conta para baixar os ROMs dos jogos do ShmupX.")
        : v.noList
          ? t("Esta pasta não tem o arquivo roms.json, então não há ROMs para baixar.")
        : v.total === 0
          ? t("Todos os ROMs da lista já estão nesta pasta.")
          : t("{done} de {total} ROMs baixados.", { done: String(v.done.length), total: String(v.total) })
      : t("Baixando {game} ({current} de {total})", { game: title(v.game) || v.rom, current: String(Math.min(v.index + 1, v.total)), total: String(v.total) });
  return (
    <div className="controller-dialog-backdrop rom-download-backdrop">
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby="rom-download-title" aria-describedby="rom-download-status" tabIndex={-1} className="controller-dialog rom-download-dialog">
        <header>
          <div>
            <h2 id="rom-download-title">{t(finished ? "ROMs prontos" : "Baixando ROMs")}</h2>
            <p>{t("Os ROMs dos jogos vão para a pasta escolhida.")}</p>
          </div>
          {finished && <DialogClose onClick={closeRomDownload} />}
        </header>
        <div className="controller-dialog-body">
          <div className="rom-download-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} aria-label={t("Andamento do download")}>
            <span className={v.stage === "reading" ? "is-indeterminate" : undefined} style={{ width: v.stage === "reading" ? undefined : `${percent}%` }} />
          </div>
          <div className="rom-download-status">
            <p id="rom-download-status" role="status">{status}</p>
            {v.stage === "downloading" && <span>{v.size ? `${megabytes(v.received)} / ${megabytes(v.size)} MB` : v.received > 0 ? `${megabytes(v.received)} MB` : `${percent}%`}</span>}
          </div>
          {finished && v.failed.length > 0 && <p className="rom-download-failed">{t("Não foi possível baixar: {roms}. Confira a internet e tente de novo mais tarde.", { roms: v.failed.map(title).join(", ") })}</p>}
        </div>
        {finished && <footer style={{ justifyContent: "flex-end" }}><button type="button" className="primary-action" onClick={closeRomDownload}>{t("Concluir")}</button></footer>}
      </div>
    </div>
  );
}
