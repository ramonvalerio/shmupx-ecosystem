import type { ReactNode } from "react";

/** Seção do menu que ainda não tem tela: título, o que ela vai ter e o aviso de que está em construção. */
export default function SoonView({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex min-w-0 flex-1 flex-col">
      <header className="px-8 pt-7 pb-5">
        <h1 className="text-xl font-semibold text-zinc-100">{title}</h1>
        <p className="mt-1 max-w-2xl text-sm text-zinc-500">{children}</p>
      </header>
      <div className="mx-8 flex min-h-56 flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-white/10 bg-zinc-950/40 px-6 py-14 text-center">
        <p className="text-sm font-medium text-zinc-200">Em construção</p>
        <p className="max-w-md text-xs text-zinc-500">Esta seção já tem lugar no menu; a tela chega numa próxima versão do Lab.</p>
      </div>
    </section>
  );
}
