import Badge from "../../components/ui/Badge";
import ProgressBar from "../../components/ui/ProgressBar";
import { isYoutubeBlock, type ValidationVideo } from "../../lib/validation";

/** Situação do vídeo: andamento (download/fila/análise) ou veredito e aceitação. */
export default function VideoStatus({ video, remaining, queue }: { video: ValidationVideo; remaining?: string; queue?: number | null }) {
  const { estado, resultado } = video;
  switch (estado.fase) {
    case "baixando":
      return estado.progresso > 0 ? (
        <ProgressBar value={estado.progresso} label={`Baixando ${Math.round(estado.progresso)}%`} detail={remaining} />
      ) : (
        <ProgressBar value={0} indeterminate label="Preparando download…" />
      );
    case "analisando":
      return estado.progresso > 0 ? (
        <ProgressBar value={estado.progresso} label={`Analisando ${Math.round(estado.progresso)}%`} detail={remaining} />
      ) : (
        <ProgressBar value={0} indeterminate label="Preparando análise…" />
      );
    case "pronto":
      return <Badge>Aguardando análise</Badge>;
    case "na_fila":
      return <Badge>{queue ? `Na fila ${queue}` : "Na fila"}</Badge>;
    case "falhou": {
      if (estado.erro === "Download cancelado.") return <Badge tone="danger">Download cancelado</Badge>;
      const blocked = isYoutubeBlock(estado.erro);
      return (
        <span className="flex max-w-80 flex-col items-start gap-1">
          <Badge tone="danger">{blocked ? "Bloqueado pelo YouTube" : "Falha no download"}</Badge>
          <span title={estado.erro} className="line-clamp-2 text-[11px] leading-snug text-red-300/80">
            {blocked ? "Escolha o navegador dos cookies do YouTube (acima) e clique em Baixar de novo." : estado.erro}
          </span>
        </span>
      );
    }
    case "concluido":
      if (!resultado) return <Badge>Aguardando análise</Badge>;
      if (resultado.erro) return <Badge tone="danger">Erro no script</Badge>;
      return (
        <span className="flex flex-wrap gap-1">
          {video.acertou === true && <Badge tone="success">Acertou</Badge>}
          {video.acertou === false && <Badge tone="danger">Errou</Badge>}
          {video.acertou === null && <Badge>Sem gabarito</Badge>}
          <Badge tone={resultado.aceito_automatico ? "info" : "warning"}>{resultado.aceito_automatico ? "Aceita" : "Revisão"}</Badge>
        </span>
      );
  }
}
