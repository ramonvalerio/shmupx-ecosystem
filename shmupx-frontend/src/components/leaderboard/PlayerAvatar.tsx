const PALETA = [
  "bg-amber-400/20 text-amber-300 border-amber-400/40",
  "bg-fuchsia-400/20 text-fuchsia-300 border-fuchsia-400/40",
  "bg-cyan-400/20 text-cyan-300 border-cyan-400/40",
  "bg-emerald-400/20 text-emerald-300 border-emerald-400/40",
  "bg-violet-400/20 text-violet-300 border-violet-400/40",
];

function corPara(seed: string): string {
  let hash = 0;
  for (let i = 0; i < seed.length; i += 1) {
    hash = (hash * 31 + seed.charCodeAt(i)) >>> 0;
  }
  return PALETA[hash % PALETA.length];
}

interface PlayerAvatarProps {
  iniciais: string;
  seed: string;
  size?: "sm" | "md";
}

/** Avatar estilizado por iniciais — sem depender de upload/URL de imagem real. */
export default function PlayerAvatar({ iniciais, seed, size = "md" }: PlayerAvatarProps) {
  const dimensao = size === "sm" ? "h-8 w-8 text-[10px]" : "h-11 w-11 text-sm";

  return (
    <div
      className={`flex ${dimensao} flex-shrink-0 items-center justify-center rounded-full border font-arcade font-bold ${corPara(seed)}`}
    >
      {iniciais.slice(0, 2).toUpperCase()}
    </div>
  );
}
