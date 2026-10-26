import { Suspense } from "react";
import JogoDetailContent from "./JogoDetailContent";

export default function JogoDetailPage(props: PageProps<"/jogos/[jogoId]">) {
  return (
    <div className="mx-auto max-w-4xl px-4 py-10 sm:px-6">
      <Suspense
        fallback={
          <div className="h-56 animate-pulse rounded-lg border border-white/10 bg-zinc-900/40" />
        }
      >
        <JogoDetailContent paramsPromise={props.params} />
      </Suspense>
    </div>
  );
}
