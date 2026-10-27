import type { ComponentType, SVGProps } from "react";
import { IconCatalog, IconLogout, IconValidation } from "../ui/Icons";

export type LabView = "catalog" | "validation";

const VIEWS: { id: LabView; label: string; icon: ComponentType<SVGProps<SVGSVGElement>> }[] = [
  { id: "catalog", label: "Catálogo", icon: IconCatalog },
  { id: "validation", label: "Validação", icon: IconValidation },
];

interface AppNavProps {
  view: LabView;
  onChangeView: (view: LabView) => void;
  accountEmail: string;
  onSignOut: () => void;
}

/** Navegação principal do Lab: seções à esquerda, conta no rodapé. */
export default function AppNav({ view, onChangeView, accountEmail, onSignOut }: AppNavProps) {
  return (
    <nav aria-label="Seções do Lab" className="flex h-full w-56 flex-shrink-0 flex-col border-r border-white/5 bg-zinc-950">
      <div className="flex items-center gap-2.5 px-5 py-5">
        <span aria-hidden className="grid h-7 w-7 place-items-center rounded-md bg-cyan-400/15 text-xs font-bold text-cyan-300 ring-1 ring-cyan-400/30">
          X
        </span>
        <div className="leading-tight">
          <p className="text-sm font-semibold text-zinc-100">ShmupX Lab</p>
          <p className="text-[11px] text-zinc-500">Backoffice</p>
        </div>
      </div>

      <ul className="flex-1 space-y-0.5 px-3">
        {VIEWS.map(({ id, label, icon: Icon }) => {
          const active = view === id;
          return (
            <li key={id}>
              <button
                type="button"
                onClick={() => onChangeView(id)}
                aria-current={active ? "page" : undefined}
                className={`flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-sm transition-colors focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none ${
                  active ? "bg-white/[0.06] font-medium text-zinc-100" : "text-zinc-400 hover:bg-white/[0.03] hover:text-zinc-200"
                }`}
              >
                <Icon className={active ? "text-cyan-300" : ""} />
                {label}
              </button>
            </li>
          );
        })}
      </ul>

      <div className="flex items-center gap-2.5 border-t border-white/5 px-4 py-3">
        <span aria-hidden className="grid h-7 w-7 flex-shrink-0 place-items-center rounded-full bg-zinc-800 text-xs font-semibold text-zinc-300 uppercase">
          {accountEmail.charAt(0)}
        </span>
        <p className="min-w-0 flex-1 truncate text-xs text-zinc-400" title={accountEmail}>{accountEmail}</p>
        <button
          type="button"
          onClick={onSignOut}
          title="Sair"
          aria-label="Sair"
          className="rounded-md p-1.5 text-zinc-500 transition-colors hover:bg-white/5 hover:text-zinc-200 focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none"
        >
          <IconLogout />
        </button>
      </div>
    </nav>
  );
}
