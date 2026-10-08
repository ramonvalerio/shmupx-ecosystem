import { useState } from "react";
import GameRoomList from "./components/GameRoomList";
import RoomView from "./components/RoomView";
import { MOCK_MESSAGES, MOCK_ROOMS } from "./data/mockRooms";
import type { ChatMessage } from "./types/room";

export default function App() {
  const [selectedRoomId, setSelectedRoomId] = useState<string | null>(null);
  const [messagesByRoom, setMessagesByRoom] = useState<Record<string, ChatMessage[]>>(MOCK_MESSAGES);

  const selectedRoom = MOCK_ROOMS.find((room) => room.id === selectedRoomId) ?? null;
  const messages = selectedRoomId ? messagesByRoom[selectedRoomId] ?? [] : [];

  function handleSendMessage(text: string) {
    if (!selectedRoomId) return;

    const newMessage: ChatMessage = {
      id: crypto.randomUUID(),
      author: "Você",
      text,
      timestamp: new Date().toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" }),
    };

    setMessagesByRoom((prev) => ({
      ...prev,
      [selectedRoomId]: [...(prev[selectedRoomId] ?? []), newMessage],
    }));
  }

  return (
    <main className="flex h-screen w-screen overflow-hidden bg-zinc-950 text-zinc-100">
      <GameRoomList
        rooms={MOCK_ROOMS}
        selectedRoomId={selectedRoomId}
        onSelectRoom={setSelectedRoomId}
      />
      <RoomView room={selectedRoom} messages={messages} onSendMessage={handleSendMessage} />
    </main>
  );
}
