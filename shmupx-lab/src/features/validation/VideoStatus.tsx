import Badge from "../../components/ui/Badge";
import type { ValidationVideo } from "../../lib/validation";

/** Situação do vídeo: andamento (download/fila/análise) ou veredito e aceitação. */
export default function VideoStatus({ video }: { video: ValidationVideo }) {
  const { estado, resultado } = video;
  switch (estado.fase) {
    case "baixando":
      return <Badge tone="info">Baixando {Math.round(estado.progresso)}%</Badge>;
    case "pronto":
      return <Badge>Aguardando análise</Badge>;
    case "na_fila":
      return <Badge>Na fila</Badge>;
    case "analisando":
      return (
        <Badge tone="info">
          <span aria-hidden className="h-2.5 w-2.5 animate-spin rounded-full border-2 border-current border-t-transparent" />
          Analisando
        </Badge>
      );
    case "falhou":
      return <Badge tone="danger">Falha no download</Badge>;
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
