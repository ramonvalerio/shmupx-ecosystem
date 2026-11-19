import type { Ambiente } from "../../lib/ambiente";

interface LoginScreenProps {
  ambiente: Ambiente;
  /** Volta à escolha do ambiente. Ausente quando `VITE_SHMUPX_ENV` fixa o ambiente. */
  onSwitchEnvironment?: () => void;
  busy: boolean;
  error?: string;
  onSignIn: () => void;
}

export default function LoginScreen({ ambiente, onSwitchEnvironment, busy, error, onSignIn }: LoginScreenProps) {
  return (
    <main className="flex h-screen w-screen items-center justify-center bg-zinc-950 text-zinc-100">
      <div className="w-96 rounded-lg border border-white/5 bg-zinc-900/50 p-8 text-center">
        <p className="text-xs font-semibold uppercase tracking-[0.25em] text-cyan-400">
          ShmupX Lab{" "}
          <span className={ambiente === "dev" ? "text-amber-300" : "text-red-300"}>· {ambiente === "dev" ? "Dev" : "Prod"}</span>
        </p>
        <h1 className="mt-3 text-lg font-semibold">Entrar</h1>
        <p className="mt-2 text-sm text-zinc-400">Use a conta Google de administrador. O navegador do sistema será aberto.</p>
        {error && <p role="alert" className="mt-4 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-left text-sm text-red-200">{error}</p>}
        <button
          type="button"
          onClick={onSignIn}
          disabled={busy}
          className="mt-6 w-full rounded-md border border-cyan-400/50 bg-cyan-400/10 px-4 py-2.5 text-sm font-medium text-cyan-200 transition-colors hover:bg-cyan-400/20 disabled:opacity-50"
        >
          {busy ? "Aguardando o navegador…" : "Entrar com Google"}
        </button>
        {onSwitchEnvironment && (
          <button type="button" onClick={onSwitchEnvironment} disabled={busy} className="mt-3 text-xs text-zinc-500 transition-colors hover:text-zinc-200 disabled:opacity-50">
            Trocar ambiente
          </button>
        )}
      </div>
    </main>
  );
}
