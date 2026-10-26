import { Suspense } from "react";
import type { Metadata } from "next";
import JogosList from "../../components/jogos/JogosList";
import JogosListSkeleton from "../../components/jogos/JogosListSkeleton";

export const metadata: Metadata = {
  title: "Jogos — ShmupX",
};

export default function JogosPage() {
  return (
    <div className="mx-auto max-w-5xl px-4 py-10 sm:px-6">
      <div className="mb-8">
        <p className="font-arcade text-[11px] uppercase tracking-[0.3em] text-cyan-400">
          Salas Ativas
        </p>
        <h1 className="mt-3 text-2xl font-semibold text-zinc-100">Shmups da Plataforma</h1>
        <p className="mt-2 text-sm text-zinc-400">
          Lista real via <code className="text-zinc-300">GET /api/jogos</code> do backend.
          Selecione um jogo para ver o placar.
        </p>
      </div>

      {/* Fetch dinâmico (sem cache) — precisa de um limite de Suspense sob Cache Components,
          ver node_modules/next/dist/docs/01-app/01-getting-started/08-caching.md */}
      <Suspense fallback={<JogosListSkeleton />}>
        <JogosList />
      </Suspense>
    </div>
  );
}
