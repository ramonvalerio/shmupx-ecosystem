import { useMemo, useState } from "react";
import GameSidebar, { type LabView } from "./components/layout/GameSidebar";
import LoginScreen from "./components/layout/LoginScreen";
import GameWorkspace from "./features/mapping/GameWorkspace";
import CatalogView from "./features/catalog/CatalogView";
import { useAdminCatalog } from "./features/catalog/useAdminCatalog";
import { useAuth } from "./auth/useAuth";
import { createDefaultMappingBoxes } from "./types/mapping";
import type { LabGame } from "./types/game";
import type { MappingBox } from "./types/mapping";
import type { ProfileVariant } from "./types/profile";

export default function App() {
  const auth = useAuth();
  const catalog = useAdminCatalog(auth.state.status === "signed-in");

  const [view, setView] = useState<LabView>("catalog");
  const [selectedGameId, setSelectedGameId] = useState<string | null>(null);
  const [variant, setVariant] = useState<ProfileVariant>("lcd");
  const [boxesByKey, setBoxesByKey] = useState<Record<string, MappingBox[]>>({});

  // O mapeamento trabalha sobre os jogos do catálogo. A versão do perfil publicado ainda não
  // existe (publicação de scripts e perfis é a fase seguinte), então fica como pendente.
  const games = useMemo<LabGame[]>(
    () =>
      catalog.games.map((game) => ({
        id: game.id,
        title: game.title,
        steamAppId: game.steam_app_id,
        profileVersion: "sem perfil",
      })),
    [catalog.games],
  );

  if (auth.state.status === "loading") return <main className="h-screen w-screen bg-zinc-950" aria-busy="true" />;
  if (auth.state.status === "signed-out") {
    return <LoginScreen busy={auth.state.busy ?? false} error={auth.state.error} onSignIn={auth.signIn} />;
  }

  const selectedGame = games.find((game) => game.id === selectedGameId) ?? games[0] ?? null;
  const mappingKey = selectedGame ? `${selectedGame.id}__${variant}` : null;
  const boxes = mappingKey ? boxesByKey[mappingKey] ?? createDefaultMappingBoxes() : [];

  function handleChangeBoxes(nextBoxes: MappingBox[]) {
    if (!mappingKey) return;
    setBoxesByKey((prev) => ({ ...prev, [mappingKey]: nextBoxes }));
  }

  return (
    <main className="flex h-screen w-screen overflow-hidden bg-zinc-950 text-zinc-100">
      <GameSidebar
        games={games}
        selectedGameId={selectedGame?.id ?? null}
        onSelectGame={(id) => { setSelectedGameId(id); setView("mapping"); }}
        view={view}
        onChangeView={setView}
        accountEmail={auth.state.email}
        onSignOut={auth.signOut}
      />

      {view === "catalog" ? (
        <CatalogView games={catalog.games} loading={catalog.loading} error={catalog.error} onReload={catalog.reload} onSaved={catalog.upsert} />
      ) : selectedGame ? (
        <GameWorkspace
          game={selectedGame}
          variant={variant}
          boxes={boxes}
          onChangeVariant={setVariant}
          onChangeBoxes={handleChangeBoxes}
        />
      ) : (
        <section className="flex flex-1 items-center justify-center text-sm text-zinc-500">
          Cadastre um jogo no Catálogo para começar o mapeamento.
        </section>
      )}
    </main>
  );
}
