import { useI18n } from "../i18n";
import type { GameRoom } from "../types/room";

interface GameRoomListProps {
  rooms: GameRoom[];
  selectedRoomId: string | null;
  onSelectRoom: (roomId: string) => void;
}

export default function GameRoomList({ rooms, selectedRoomId, onSelectRoom }: GameRoomListProps) {
  const { t } = useI18n();
  return (
    <aside className="flex h-full w-80 flex-shrink-0 flex-col border-r border-white/5 bg-zinc-950">
      <div className="border-b border-white/5 px-5 py-5">
        <p className="font-arcade text-[10px] text-emerald-400">{t("SHMUPX")}</p>
        <h2 className="mt-1 text-xs font-semibold uppercase tracking-[0.25em] text-zinc-500">
          {t(" Salas Ativas ")}</h2>
      </div>

      <ul className="scrollbar-arcade flex-1 space-y-2 overflow-y-auto px-3 py-4">
        {rooms.map((room) => {
          const isSelected = room.id === selectedRoomId;
          return (
            <li
              key={room.id}
              className={`rounded-lg border px-4 py-3 transition-colors ${
                isSelected
                  ? "border-emerald-400/60 bg-emerald-400/10 shadow-[0_0_24px_-6px_rgba(16,185,129,0.65)]"
                  : "border-white/5 bg-zinc-900/60"
              }`}
            >
              <p className={`text-sm font-semibold ${isSelected ? "text-emerald-300" : "text-zinc-100"}`}>
                {room.title}
              </p>
              <p className="mt-1 text-xs text-zinc-500">{room.playersOnline} {t(" online agora")}</p>
              <button
                type="button"
                onClick={() => onSelectRoom(room.id)}
                className={`mt-3 w-full rounded-md border px-3 py-1.5 text-[11px] font-semibold uppercase tracking-wider transition-colors ${
                  isSelected
                    ? "border-emerald-400 bg-emerald-400/20 text-emerald-200"
                    : "border-fuchsia-400/50 text-fuchsia-300 hover:bg-fuchsia-400/10"
                }`}
              >
                {isSelected ? t("Na Sala") : t("Entrar na Sala")}
              </button>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}
