import { useCallback, useMemo, useState } from "react";
import AppNav, { type LabView } from "./components/layout/AppNav";
import GameSidebar, { type SidebarGame } from "./components/layout/GameSidebar";
import EnvironmentScreen from "./components/layout/EnvironmentScreen";
import LoginScreen from "./components/layout/LoginScreen";
import Button from "./components/ui/Button";
import CatalogView from "./features/catalog/CatalogView";
import CoinsView from "./features/coins/CoinsView";
import ClientView from "./features/client/ClientView";
import UsersView from "./features/users/UsersView";
import { useUsers } from "./features/users/useUsers";
import { useReports } from "./features/users/useReports";
import { useAdminCatalog } from "./features/catalog/useAdminCatalog";
import ReleasesView from "./features/releases/ReleasesView";
import { useReleases } from "./features/releases/useReleases";
import ValidationView from "./features/validation/ValidationView";
import { useValidationEnvironment } from "./features/validation/useValidation";
import { defaultScript, scriptsOf } from "./lib/validation";
import { useAuth } from "./auth/useAuth";
import { getAmbiente, iniciarFirebase, trocarAmbiente } from "./auth/firebase";
import { AMBIENTE_FORCADO, lembrarAmbiente, ultimoAmbiente, type Ambiente } from "./lib/ambiente";

/**
 * Ao abrir, o Lab pergunta o ambiente (Dev ou Prod) e só então inicia o Firebase e o login.
 * `VITE_SHMUPX_ENV` fixa o ambiente e pula a pergunta. Trocar = sair e recarregar.
 */
export default function App() {
  const [ambiente, setAmbiente] = useState<Ambiente | null>(() => {
    if (!AMBIENTE_FORCADO) return null;
    iniciarFirebase(AMBIENTE_FORCADO);
    return AMBIENTE_FORCADO;
  });

  if (!ambiente) {
    return (
      <EnvironmentScreen
        preselected={ultimoAmbiente()}
        onChoose={(escolhido) => {
          lembrarAmbiente(escolhido);
          iniciarFirebase(escolhido);
          setAmbiente(escolhido);
        }}
      />
    );
  }
  return <Lab onSwitchEnvironment={AMBIENTE_FORCADO ? undefined : () => void trocarAmbiente()} />;
}

function Lab({ onSwitchEnvironment }: { onSwitchEnvironment?: () => void }) {
  const auth = useAuth();
  const signedIn = auth.state.status === "signed-in";
  const catalog = useAdminCatalog(signedIn);

  const [view, setView] = useState<LabView>("catalog");
  const [selectedGameId, setSelectedGameId] = useState<string | null>(null);
  // Python, yt-dlp e scripts: verificado na primeira vez que a Validação (ou Versões, que lista
  // os scripts aprovados) abre.
  const validation = useValidationEnvironment(signedIn && (view === "validation" || view === "releases"));
  const releases = useReleases(signedIn && view === "releases", validation.environment?.scripts);
  // Jogadores: no Users Manager e no Coins Manager (escolher quem recebe as fichas).
  const users = useUsers(signedIn && (view === "users" || view === "coins"));
  const reports = useReports(signedIn && view === "users");
  const [coinsFor, setCoinsFor] = useState<string | null>(null);
  const clearCoinsFor = useCallback(() => setCoinsFor(null), []);

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
    return <LoginScreen ambiente={getAmbiente()} onSwitchEnvironment={onSwitchEnvironment} busy={auth.state.busy ?? false} error={auth.state.error} onSignIn={auth.signIn} />;
  }

  const selectedGame = games.find((game) => game.id === selectedGameId) ?? games[0] ?? null;
  const selectedScripts = selectedGame ? scriptsOf(validation.environment?.scripts ?? [], selectedGame.id) : [];

  return (
    <main className="flex h-screen w-screen overflow-hidden bg-zinc-950 text-zinc-100 antialiased">
      <AppNav view={view} onChangeView={setView} accountEmail={auth.state.email} onSignOut={auth.signOut} onSwitchEnvironment={onSwitchEnvironment} />

      {view === "catalog" ? (
        <CatalogView games={catalog.games} loading={catalog.loading} error={catalog.error} onReload={catalog.reload} onSaved={catalog.upsert} onDeleted={catalog.remove} />
      ) : view === "client" ? (
        <ClientView />
      ) : view === "coins" ? (
        <CoinsView users={users.users} usersLoading={users.loading} preselectedUid={coinsFor} onPreselectedUsed={clearCoinsFor} />
      ) : view === "users" ? (
        <UsersView
          users={users.users}
          loading={users.loading}
          error={users.error}
          updatedAt={users.updatedAt}
          onReload={() => void users.reload()}
          onModerated={users.setModeration}
          reports={reports.reports}
          onReportChanged={(report) => {
            reports.replace(report);
            // O contador de denúncias abertas da lista de jogadores vem do backend.
            void users.reload(true);
          }}
          onSendCoins={(uid) => {
            setCoinsFor(uid);
            setView("coins");
          }}
        />
      ) : view === "releases" ? (
        <ReleasesView
          releases={releases.releases}
          loading={releases.loading}
          error={releases.error}
          scripts={validation.environment?.scripts ?? []}
          onReload={() => void releases.reload()}
          onSaved={releases.upsert}
        />
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
