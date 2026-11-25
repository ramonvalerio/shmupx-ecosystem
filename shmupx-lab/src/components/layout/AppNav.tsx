import type { ComponentType, SVGProps } from "react";
import { IconCalendar, IconCatalog, IconCoins, IconLogout, IconMonitor, IconPlans, IconRelease, IconUsers, IconValidation } from "../ui/Icons";
import { getAmbiente } from "../../auth/firebase";

export type LabView = "client" | "plans" | "releases" | "users" | "coins" | "events" | "catalog" | "validation";

type NavItem = { id: LabView; label: string; icon: ComponentType<SVGProps<SVGSVGElement>> };

/** Menu do Lab em seções: geral (Client e versões), controle (jogadores, fichas, eventos) e jogos. */
const SECTIONS: { title: string; items: NavItem[] }[] = [
  {
    title: "General section",
    items: [
      { id: "client", label: "Client", icon: IconMonitor },
      { id: "plans", label: "Plans", icon: IconPlans },
      { id: "releases", label: "Versions", icon: IconRelease },
    ],
  },
  {
    title: "Control",
    items: [
      { id: "users", label: "Users", icon: IconUsers },
      { id: "coins", label: "Coins", icon: IconCoins },
      { id: "events", label: "Events", icon: IconCalendar },
    ],
  },
  {
    title: "Game section",
    items: [
      { id: "catalog", label: "Catálogo", icon: IconCatalog },
      { id: "validation", label: "Validação", icon: IconValidation },
    ],
  },
];

interface AppNavProps {
  view: LabView;
  onChangeView: (view: LabView) => void;
  accountEmail: string;
  onSignOut: () => void;
  /** Sai e volta à escolha do ambiente. Ausente quando `VITE_SHMUPX_ENV` fixa o ambiente. */
  onSwitchEnvironment?: () => void;
}

/** Navegação principal do Lab: seções à esquerda, conta no rodapé. */
export default function AppNav({ view, onChangeView, accountEmail, onSignOut, onSwitchEnvironment }: AppNavProps) {
  const ambiente = getAmbiente();
  return (
    <nav aria-label="Seções do Lab" className="flex h-full w-56 flex-shrink-0 flex-col border-r border-white/5 bg-zinc-950">
      <div className="flex items-center gap-2.5 px-5 py-5">
        <span aria-hidden className="grid h-7 w-7 place-items-center rounded-md bg-cyan-400/15 text-xs font-bold text-cyan-300 ring-1 ring-cyan-400/30">
          X
        </span>
        <div className="leading-tight">
          <p className="flex items-center gap-1.5 text-sm font-semibold text-zinc-100">
            ShmupX Lab
            <span
              className={`rounded px-1.5 py-px text-[9px] font-bold tracking-wider ring-1 ${
                ambiente === "dev" ? "bg-amber-500/15 text-amber-200 ring-amber-400/40" : "bg-red-500/15 text-red-200 ring-red-400/40"
              }`}
            >
              {ambiente === "dev" ? "DEV" : "PROD"}
            </span>
          </p>
          <p className="text-[11px] text-zinc-500">Backoffice</p>
        </div>
      </div>
      {ambiente === "dev" ? (
        <p className="mx-3 mb-3 rounded-md border border-amber-400/30 bg-amber-500/10 px-3 py-2 text-[11px] leading-snug text-amber-200" title="Firestore e login nos emuladores do Docker (projeto demo-shmupx). Nada aqui chega aos jogadores.">
          <strong className="block tracking-wider">DESENVOLVIMENTO</strong>
          Emuladores locais · sem jogadores reais
        </p>
      ) : (
        <p className="mx-3 mb-3 rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-[11px] leading-snug text-red-200">
          <strong className="block tracking-wider">PRODUÇÃO</strong>
          Mudanças valem para os jogadores
        </p>
      )}

      {onSwitchEnvironment && (
        <button
          type="button"
          onClick={onSwitchEnvironment}
          className="mx-3 mb-3 rounded-md px-3 py-1 text-left text-[11px] text-zinc-500 transition-colors hover:bg-white/[0.03] hover:text-zinc-200 focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none"
        >
          Trocar ambiente
        </button>
      )}

      <div className="flex-1 overflow-y-auto px-3">
        {SECTIONS.map((section, index) => (
          <section key={section.title} aria-label={section.title} className={index > 0 ? "mt-3 border-t border-white/5 pt-3" : ""}>
            <p className="mb-1 px-3 text-[10px] font-semibold tracking-[0.14em] text-zinc-600 uppercase">{section.title}</p>
            <ul className="space-y-0.5">
              {section.items.map(({ id, label, icon: Icon }) => {
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
          </section>
        ))}
      </div>

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
