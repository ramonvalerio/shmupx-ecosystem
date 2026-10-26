import Link from "next/link";
import type { Jogo } from "../../types/jogo";

interface JogoCardProps {
  jogo: Jogo;
}

export default function JogoCard({ jogo }: JogoCardProps) {
  return (
    <Link
      href={`/jogos/${jogo.id}`}
      className="group flex flex-col gap-2 rounded-lg border border-white/10 bg-zinc-900/60 p-5 transition-colors hover:border-fuchsia-400/40 hover:bg-zinc-900"
    >
      <p className="text-xs uppercase tracking-wider text-zinc-500">
        Steam App ID {jogo.steam_app_id}
      </p>
      <h3 className="text-lg font-semibold text-zinc-100 group-hover:text-fuchsia-300">
        {jogo.nome}
      </h3>
      <span className="mt-2 text-xs font-semibold uppercase tracking-wider text-cyan-300">
        Ver placar →
      </span>
    </Link>
  );
}
