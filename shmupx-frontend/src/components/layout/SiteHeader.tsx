import Link from "next/link";

export default function SiteHeader() {
  return (
    <header className="sticky top-0 z-10 border-b border-white/10 bg-zinc-950/90 backdrop-blur">
      <div className="mx-auto flex max-w-5xl items-center justify-between gap-4 px-4 py-4 sm:px-6">
        <Link href="/" className="flex items-baseline gap-2">
          <span className="font-arcade text-sm text-amber-400 sm:text-base">SHMUPX</span>
          <span className="hidden text-xs uppercase tracking-[0.3em] text-zinc-500 sm:inline">
            leaderboard
          </span>
        </Link>

        <nav className="flex items-center gap-4 text-xs font-semibold uppercase tracking-wider sm:text-sm">
          <Link href="/" className="text-zinc-300 transition-colors hover:text-amber-300">
            Ranking Global
          </Link>
          <Link href="/jogos" className="text-zinc-300 transition-colors hover:text-fuchsia-300">
            Jogos
          </Link>
        </nav>
      </div>
    </header>
  );
}
