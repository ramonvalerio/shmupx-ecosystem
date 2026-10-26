import type { JogadorRanking } from "../../types/ranking";
import PlayerAvatar from "./PlayerAvatar";

const MEDALHA = ["🥇", "🥈", "🥉"];
const ALTURA = ["h-28", "h-20", "h-14"];
const BORDA = [
  "border-amber-400/70 shadow-[0_0_30px_-8px_rgba(251,191,36,0.7)]",
  "border-zinc-300/50",
  "border-orange-500/50",
];
/** Ordem visual do pódio: 2º à esquerda, 1º no centro (mais alto), 3º à direita. */
const ORDEM_VISUAL = [1, 0, 2];

interface PodiumTop3Props {
  top3: JogadorRanking[];
}

export default function PodiumTop3({ top3 }: PodiumTop3Props) {
  if (top3.length < 3) return null;

  return (
    <div className="flex items-end justify-center gap-4 pb-2 sm:gap-8">
      {ORDEM_VISUAL.map((indiceOriginal) => {
        const jogador = top3[indiceOriginal];
        return (
          <div key={jogador.id} className="flex flex-col items-center gap-2">
            <span className="text-2xl">{MEDALHA[indiceOriginal]}</span>
            <PlayerAvatar iniciais={jogador.avatarIniciais} seed={jogador.id} />
            <p className="font-arcade text-[11px] text-zinc-200">{jogador.nome}</p>
            <p className="font-arcade text-base text-amber-300">
              {jogador.pontuacaoGlobalNormalizada.toFixed(2)}
            </p>
            <div
              className={`w-20 rounded-t-md border-x border-t bg-zinc-900/80 sm:w-28 ${ALTURA[indiceOriginal]} ${BORDA[indiceOriginal]}`}
            />
          </div>
        );
      })}
    </div>
  );
}
