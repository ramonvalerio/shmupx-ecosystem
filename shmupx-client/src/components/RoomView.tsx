import ChatFeed from "./ChatFeed";
import ChatInput from "./ChatInput";
import type { ChatMessage, GameRoom } from "../types/room";

interface RoomViewProps {
  room: GameRoom | null;
  messages: ChatMessage[];
  onSendMessage: (text: string) => void;
}

const MOCK_FICHAS_DIGITAIS = 3;

export default function RoomView({ room, messages, onSendMessage }: RoomViewProps) {
  if (!room) {
    return (
      <section className="flex flex-1 flex-col items-center justify-center gap-2 bg-zinc-900/40 text-center">
        <p className="font-arcade text-sm text-fuchsia-400">SHMUPX</p>
        <p className="text-sm text-zinc-500">Selecione uma sala para entrar na ação.</p>
      </section>
    );
  }

  const activeRoom = room;

  function handleStartGame() {
    // TODO: integrar com o comando Tauri `inicializar_jogo_steam` (ver src-tauri/src/steam/commands.rs).
    console.log("iniciar jogo via Steam:", activeRoom.id);
  }

  return (
    <section className="flex flex-1 flex-col overflow-hidden bg-zinc-900/40">
      <header className="flex items-center justify-between border-b border-white/5 px-6 py-4">
        <div>
          <p className="text-[10px] uppercase tracking-[0.25em] text-zinc-500">Sala ativa</p>
          <h1 className="text-lg font-bold text-zinc-100">{room.title}</h1>
        </div>

        <div className="flex items-center gap-4">
          <div className="rounded-md border border-fuchsia-400/40 bg-fuchsia-400/10 px-3 py-1.5 text-xs font-semibold text-fuchsia-300">
            {MOCK_FICHAS_DIGITAIS} Fichas Digitais
          </div>
          <button
            type="button"
            onClick={handleStartGame}
            className="rounded-md border border-emerald-400 bg-emerald-400/10 px-4 py-1.5 text-xs font-semibold uppercase tracking-wider text-emerald-300 shadow-[0_0_20px_-6px_rgba(16,185,129,0.7)] transition-colors hover:bg-emerald-400/20"
          >
            Iniciar Jogo via Steam
          </button>
        </div>
      </header>

      <ChatFeed messages={messages} />
      <ChatInput onSendMessage={onSendMessage} />
    </section>
  );
}
