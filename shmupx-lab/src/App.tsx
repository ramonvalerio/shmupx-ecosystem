import { useMemo, useState } from "react";
import AppNav, { type LabView } from "./components/layout/AppNav";
import GameSidebar, { type SidebarGame } from "./components/layout/GameSidebar";
import LoginScreen from "./components/layout/LoginScreen";
import Button from "./components/ui/Button";
import CatalogView from "./features/catalog/CatalogView";
import { useAdminCatalog } from "./features/catalog/useAdminCatalog";
import ValidationView from "./features/validation/ValidationView";
import { useValidationEnvironment } from "./features/validation/useValidation";
import { defaultScript, scriptsOf } from "./lib/validation";
import { useAuth } from "./auth/useAuth";

export default function App() {
  const auth = useAuth();
  const signedIn = auth.state.status === "signed-in";
  const catalog = useAdminCatalog(signedIn);

  const [view, setView] = useState<LabView>("catalog");
  const [selectedGameId, setSelectedGameId] = useState<string | null>(null);
  // Python, yt-dlp e scripts: verificado na primeira vez que a Validação abre.
  const validation = useValidationEnvironment(signedIn && view === "validation");

  const games = useMemo<SidebarGame[]>(() => {
    const scripts = validation.environment?.scripts ?? [];
    return catalog.games.map((game) => ({
      id: game.id,
      title: game.title,
      scriptVersion: defaultScript(scriptsOf(scripts, game.id))?.version ?? null,
    }));
  }, [catalog.games, validation.environment]);

  if (auth.state.status === "loading") return <main className="h-screen w-screen bg-zinc-950" aria-busy="true" />;
  if (auth.state.status === "signed-out") {
    return <LoginScreen busy={auth.state.busy ?? false} error={auth.state.error} onSignIn={auth.signIn} />;
  }

  const selectedGame = games.find((game) => game.id === selectedGameId) ?? games[0] ?? null;
  const selectedScripts = selectedGame ? scriptsOf(validation.environment?.scripts ?? [], selectedGame.id) : [];

  return (
    <main className="flex h-screen w-screen overflow-hidden bg-zinc-950 text-zinc-100 antialiased">
      <AppNav view={view} onChangeView={setView} accountEmail={auth.state.email} onSignOut={auth.signOut} />

      {view === "catalog" ? (
        <CatalogView games={catalog.games} loading={catalog.loading} error={catalog.error} onReload={catalog.reload} onSaved={catalog.upsert} />
      ) : selectedGame ? (
        <>
          <GameSidebar games={games} selectedGameId={selectedGame.id} onSelectGame={setSelectedGameId} />
          <ValidationView
            gameId={selectedGame.id}
            gameTitle={selectedGame.title}
            scripts={selectedScripts}
            environment={validation.environment}
            checkingEnvironment={validation.checking}
            onCheckEnvironment={() => void validation.check()}
          />
        </>
      ) : (
        <section className="flex flex-1 flex-col items-center justify-center gap-3 text-center">
          <p className="text-sm font-medium text-zinc-200">Nenhum jogo para validar</p>
          <p className="max-w-sm text-xs text-zinc-500">A validação usa os jogos do catálogo. Cadastre um jogo primeiro.</p>
          <Button variant="primary" size="sm" onClick={() => setView("catalog")} className="mt-2">
            Ir para o Catálogo
          </Button>
        </section>
      )}
    </main>
  );
}
