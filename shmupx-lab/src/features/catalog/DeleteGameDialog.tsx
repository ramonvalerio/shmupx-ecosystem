import { useEffect, useState } from "react";
import Button from "../../components/ui/Button";
import { TextInput } from "../../components/ui/Field";
import { IconAlert, IconClose } from "../../components/ui/Icons";
import { deleteGame, type CatalogGame } from "../../lib/backend";

interface DeleteGameDialogProps {
  game: CatalogGame;
  onClose: () => void;
  /** Chamado depois que o backend confirmou a exclusão. */
  onDeleted: () => void;
}

/** Confirma a exclusão de um jogo do catálogo. Pede o ID digitado: não há como desfazer. */
export default function DeleteGameDialog({ game, onClose, onDeleted }: DeleteGameDialogProps) {
  const [typed, setTyped] = useState("");
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const confirmed = typed.trim() === game.id;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !deleting) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, deleting]);

  async function remove() {
    if (!confirmed) return;
    setDeleting(true);
    setError(null);
    try {
      await deleteGame(game.id);
      onDeleted();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setDeleting(false);
    }
  }

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/70 p-6" onClick={() => !deleting && onClose()}>
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="excluir-jogo-titulo"
        onClick={(event) => event.stopPropagation()}
        className="flex max-h-full w-full max-w-md flex-col overflow-hidden rounded-lg border border-white/10 bg-[#0e0e11] shadow-2xl shadow-black/60"
      >
        <header className="flex items-start justify-between gap-4 border-b border-white/5 px-6 py-4">
          <div className="min-w-0">
            <p className="text-[11px] font-semibold tracking-wider text-red-300 uppercase">Excluir jogo</p>
            <h2 id="excluir-jogo-titulo" className="mt-0.5 truncate text-base font-semibold text-zinc-100">{game.title}</h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            disabled={deleting}
            aria-label="Fechar"
            className="rounded-md p-1.5 text-zinc-500 transition-colors hover:bg-white/5 hover:text-zinc-200"
          >
            <IconClose />
          </button>
        </header>

        <form
          onSubmit={(event) => {
            event.preventDefault();
            void remove();
          }}
          className="space-y-4 overflow-y-auto px-6 py-5 text-sm text-zinc-300"
        >
          <ul className="list-disc space-y-1.5 pl-5 text-xs leading-relaxed">
            {game.published && <li>O jogo está publicado: a sala some do Client na hora e novas runs ficam bloqueadas.</li>}
            <li>O cadastro e as perguntas do RobotX são apagados, sem como desfazer.</li>
            <li>Runs e rankings já gravados continuam guardados. Recriar o jogo com o mesmo ID os traz de volta.</li>
          </ul>
          <label className="block">
            <span className="mb-1.5 block text-xs text-zinc-400">
              Para confirmar, digite o ID <code className="font-mono text-zinc-200">{game.id}</code>
            </span>
            <TextInput
              autoFocus
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              className="font-mono"
              autoComplete="off"
              spellCheck={false}
            />
          </label>
          {error && (
            <p role="alert" className="flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-200">
              <IconAlert className="mt-px flex-shrink-0" />
              {error}
            </p>
          )}
          <div className="flex justify-end gap-3 pt-1">
            <Button variant="info" onClick={onClose} disabled={deleting}>Cancelar</Button>
            <Button variant="danger" type="submit" loading={deleting} disabled={!confirmed}>Excluir jogo</Button>
          </div>
        </form>
      </div>
    </div>
  );
}
