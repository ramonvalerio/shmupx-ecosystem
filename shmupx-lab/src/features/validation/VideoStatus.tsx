import Badge from "../../components/ui/Badge";
import ProgressBar from "../../components/ui/ProgressBar";
import type { ValidationVideo } from "../../lib/validation";

/** Situação do vídeo: andamento (download/fila/análise) ou veredito e aceitação. */
export default function VideoStatus({ video, remaining }: { video: ValidationVideo; remaining?: string }) {
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
      return <Badge>Na fila</Badge>;
    case "falhou":
      return <Badge tone="danger">{estado.erro === "Download cancelado." ? "Download cancelado" : "Falha no download"}</Badge>;
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
