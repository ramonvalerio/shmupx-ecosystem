import { useI18n } from "../i18n";
import { useLibrarySync } from "../data/librarySync";

/**
 * Barra de baixo: a importação automática dos jogos (Steam, ROMs e MiSTer) a cada login. Passando
 * o mouse, o que foi lido de cada lugar.
 */
export default function LibrarySyncStatus() {
  const { t } = useI18n();
  const sync = useLibrarySync();
  if (sync.running) return <span className="library-sync is-running" role="status"><i className="status-dot" />{t("Atualizando seus jogos…")}</span>;
  if (!sync.finishedAt) return null;
  const time = new Date(sync.finishedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const detail = [
    sync.steam !== null && t("Steam: {count} jogos instalados", { count: sync.steam }),
    sync.emulator !== null && t("Emulador: {count} ROMs", { count: sync.emulator }),
    sync.mister === "offline" ? t("MiSTer: não respondeu (desligado ou fora da rede)") : sync.mister !== null && t("MiSTer: {count} jogos", { count: sync.mister }),
  ].filter(Boolean).join(" · ");
  return <span className="library-sync" role="status" title={detail || undefined}>
    {t("Jogos atualizados às {time}", { time })}
    {sync.mister === "offline" && <> · {t("MiSTer não respondeu")}</>}
  </span>;
}
