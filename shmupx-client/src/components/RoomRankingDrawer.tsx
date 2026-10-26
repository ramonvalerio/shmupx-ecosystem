import { useI18n } from "../i18n";
import type { RoomRankingEntry } from "../types/ranking";
import { useDialogFocus } from "./useDialogFocus";

interface RoomRankingDrawerProps {
  open: boolean;
  gameTitle: string;
  entries: RoomRankingEntry[];
  onClose: () => void;
}

/** Gaveta lateral com o ranking online da sala (mockado — ver data/mockRoomRanking.ts). */
export default function RoomRankingDrawer({
  open,
  gameTitle,
  entries,
  onClose,
}: RoomRankingDrawerProps) {
  const { t, locale } = useI18n();
  const dialogRef = useDialogFocus(open, onClose);
  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      <button
        type="button"
        aria-label={t("Fechar ranking")}
        onClick={onClose}
        className="absolute inset-0 bg-black/70"
      />

      <div ref={dialogRef} role="dialog" aria-modal="true" aria-label={t("Ranking de {game}", {game:gameTitle})} tabIndex={-1} className="relative flex h-full w-full max-w-md flex-col border-l border-emerald-400/30 bg-zinc-950 shadow-[0_0_40px_-10px_rgba(16,185,129,0.5)]">
        <header className="flex items-start justify-between border-b border-white/10 px-5 py-4">
          <div>
            <p className="font-arcade text-[10px] uppercase tracking-[0.2em] text-emerald-400">
              {t(" Ranking Online ")}</p>
            <h2 className="mt-1 text-sm font-semibold text-zinc-100">{gameTitle}</h2>
          </div>
          <button
            type="button"
            data-dialog-close
            onClick={onClose}
            className="rounded-md border border-white/10 px-2 py-1 text-xs text-zinc-400 transition-colors hover:border-red-400/40 hover:text-red-300"
          >
            {t(" Fechar ")}</button>
        </header>

        <div className="flex-1 overflow-y-auto px-3 py-3">
          <table className="w-full border-collapse text-left text-xs">
            <thead>
              <tr className="border-b border-white/10 text-[10px] uppercase tracking-wider text-zinc-500">
                <th className="px-2 py-2 font-semibold">#</th>
                <th className="px-2 py-2 font-semibold">{t("Jogador")}</th>
                <th className="px-2 py-2 font-semibold">{t("Placar")}</th>
                <th className="px-2 py-2 font-semibold">{t("Vidas")}</th>
                <th className="px-2 py-2 font-semibold">{t("Continues")}</th>
                <th className="px-2 py-2 font-semibold">{t("Nota")}</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((entrada, index) => (
                <tr
                  key={entrada.jogadorNome}
                  className="border-b border-white/5 last:border-b-0 hover:bg-white/5"
                >
                  <td className="px-2 py-2 font-arcade text-[10px] text-zinc-500">{index + 1}</td>
                  <td className="px-2 py-2 font-medium text-zinc-100">{entrada.jogadorNome}</td>
                  <td className="px-2 py-2 font-arcade text-[11px] text-emerald-300">
                    {entrada.maiorPlacarBruto.toLocaleString(locale)}
                  </td>
                  <td className="px-2 py-2 text-zinc-400">{entrada.vidasPerdidas}</td>
                  <td className="px-2 py-2">
                    {entrada.continuesUsados > 0 ? (
                      <span className="rounded-full border border-amber-500/40 bg-amber-500/10 px-2 py-0.5 text-[10px] text-amber-300">
                        {entrada.continuesUsados}{t("x ")}</span>
                    ) : (
                      <span className="text-[10px] text-zinc-600">{t("1CC")}</span>
                    )}
                  </td>
                  <td className="px-2 py-2 font-arcade text-[11px] text-amber-300">
                    {entrada.notaNormalizada.toFixed(2)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="mt-4 text-center text-[11px] text-zinc-600">
            {t(" Dados de demonstração. O ranking online ainda não está disponível. ")}</p>
        </div>
      </div>
    </div>
  );
}
