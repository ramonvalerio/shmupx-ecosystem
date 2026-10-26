import { useState } from "react";
import GameSidebar from "./components/layout/GameSidebar";
import GameWorkspace from "./features/mapping/GameWorkspace";
import { MOCK_GAMES } from "./data/mockGames";
import { createDefaultMappingBoxes } from "./types/mapping";
import type { MappingBox } from "./types/mapping";
import type { ProfileVariant } from "./types/profile";

export default function App() {
  const [selectedGameId, setSelectedGameId] = useState<string | null>(MOCK_GAMES[0]?.id ?? null);
  const [variant, setVariant] = useState<ProfileVariant>("lcd");
  const [boxesByKey, setBoxesByKey] = useState<Record<string, MappingBox[]>>({});

  const selectedGame = MOCK_GAMES.find((game) => game.id === selectedGameId) ?? null;
  const mappingKey = selectedGame ? `${selectedGame.id}__${variant}` : null;
  const boxes = mappingKey ? boxesByKey[mappingKey] ?? createDefaultMappingBoxes() : [];

  function handleChangeBoxes(nextBoxes: MappingBox[]) {
    if (!mappingKey) return;
    setBoxesByKey((prev) => ({ ...prev, [mappingKey]: nextBoxes }));
  }

  return (
    <main className="flex h-screen w-screen overflow-hidden bg-zinc-950 text-zinc-100">
      <GameSidebar
        games={MOCK_GAMES}
        selectedGameId={selectedGameId}
        onSelectGame={setSelectedGameId}
      />

      {selectedGame ? (
        <GameWorkspace
          game={selectedGame}
          variant={variant}
          boxes={boxes}
          onChangeVariant={setVariant}
          onChangeBoxes={handleChangeBoxes}
        />
      ) : (
        <section className="flex flex-1 items-center justify-center text-sm text-zinc-500">
          Selecione um jogo para começar o mapeamento.
        </section>
      )}
    </main>
  );
}
