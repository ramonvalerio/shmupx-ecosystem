import type { MouseEvent } from "react";
import Button from "../../components/ui/Button";
import CopyButton from "../../components/ui/CopyButton";
import { IconExternal } from "../../components/ui/Icons";
import { canDownloadAgain, isBusy, openInBrowser, type ValidationVideo } from "../../lib/validation";

/** O vídeo pode ir para a fila de análise. */
export function runnable(video: ValidationVideo): boolean {
  return video.caminho !== null && (video.estado.fase === "pronto" || video.estado.fase === "concluido");
}

/** Copiar e abrir no navegador o link do YouTube. Não abrem o painel da linha. */
export function LinkActions({ url }: { url: string }) {
  function open(event: MouseEvent) {
    event.stopPropagation();
    void openInBrowser(url);
  }
  return (
    <span className="inline-flex flex-shrink-0 items-center">
      <CopyButton value={url} label="Copiar link do vídeo" />
      <button
        type="button"
        onClick={open}
        title="Abrir no navegador"
        aria-label="Abrir no navegador"
        className="inline-flex h-6 w-6 items-center justify-center rounded text-zinc-500 transition-colors hover:bg-white/5 hover:text-zinc-200 focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none"
      >
        <IconExternal width={14} height={14} />
      </button>
    </span>
  );
}

interface PrimaryActionProps {
  video: ValidationVideo;
  canRun: boolean;
  onRun: () => void;
  onCancel: () => void;
  onDownloadAgain: () => void;
  variant?: "ghost" | "primary";
  size?: "sm" | "md";
}

/** A ação que faz sentido agora: cancelar o que está em andamento, baixar de novo ou rodar. */
export function PrimaryAction({ video, canRun, onRun, onCancel, onDownloadAgain, variant = "ghost", size = "sm" }: PrimaryActionProps) {
  const stop = (action: () => void) => (event: MouseEvent) => {
    event.stopPropagation();
    action();
  };
  if (isBusy(video)) {
    return (
      <Button size={size} variant="ghost" onClick={stop(onCancel)} className="text-red-300 hover:bg-red-500/10 hover:text-red-200">
        Cancelar
      </Button>
    );
  }
  if (canDownloadAgain(video)) {
    return <Button size={size} variant={variant} onClick={stop(onDownloadAgain)}>Baixar de novo</Button>;
  }
  return (
    <Button size={size} variant={variant} disabled={!canRun || !runnable(video)} onClick={stop(onRun)}>
      {video.estado.fase === "concluido" ? "Rodar de novo" : variant === "primary" ? "Rodar script" : "Rodar"}
    </Button>
  );
}
