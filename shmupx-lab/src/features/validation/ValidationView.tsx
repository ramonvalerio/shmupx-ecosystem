import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import { IconAlert, IconCheck, IconPlus, IconRefresh } from "../../components/ui/Icons";
import {
  installYtDlp,
  cancelProcessing,
  downloadAgain,
  addLocalVideos,
  addYoutubeVideo,
  percent,
  removeVideo,
  runVideos,
  setExpected,
  type Expected,
  type ScriptInfo,
  type ValidationEnvironment,
  type ValidationVideo,
} from "../../lib/validation";
import { useRemainingTimes, useValidationDataset } from "./useValidation";
import { runnable } from "./VideoActions";
import VideoPanel from "./VideoPanel";
import VideoTable from "./VideoTable";

interface ValidationViewProps {
  gameId: string;
  gameTitle: string;
  /** Script registrado para o jogo; `null` = o jogo ainda não tem validador. */
  script: ScriptInfo | null;
  environment: ValidationEnvironment | null;
  checkingEnvironment: boolean;
  onCheckEnvironment: () => void;
}

function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function Metric({ label, value, detail, hint }: { label: string; value: ReactNode; detail: string; hint: string }) {
  return (
    <div className="rounded-lg border border-white/5 bg-zinc-950/60 px-4 py-3">
      <p className="text-xs text-zinc-500">{label}</p>
      <p className="mt-1 text-2xl font-semibold text-zinc-100 tabular-nums">{value}</p>
      <p className="mt-0.5 text-xs text-zinc-400 tabular-nums">{detail}</p>
      <p className="mt-1 text-[11px] text-zinc-600">{hint}</p>
    </div>
  );
}

/** O que falta instalar, com o comando de cada item (e instalação automática do yt-dlp). */
function EnvironmentNotice({ environment, checking, onCheck }: { environment: ValidationEnvironment; checking: boolean; onCheck: () => void }) {
  const [installing, setInstalling] = useState(false);
  const [installError, setInstallError] = useState<string | null>(null);

  async function install() {
    setInstalling(true);
    setInstallError(null);
    try {
      await installYtDlp();
      onCheck();
    } catch (e) {
      setInstallError(describe(e));
    } finally {
      setInstalling(false);
    }
  }

  const issues: { title: string; detail: string; command: string; action?: ReactNode }[] = [];
  if (!environment.python.ok) {
    issues.push({ title: "Python não disponível para o script", detail: environment.python.detalhe, command: "winget install Python.Python.3.12" });
  } else if (!environment.dependencias.ok) {
    issues.push({
      title: "Ambiente do script incompleto (OpenCV/NumPy)",
      detail: environment.dependencias.detalhe,
      command: "cd shmupx_offline; py -3.12 -m venv .venv; .\\.venv\\Scripts\\pip install -r requirements.txt",
    });
  }
  if (!environment.ytdlp.ok) {
    issues.push({
      title: "yt-dlp não instalado (necessário só para vídeos do YouTube)",
      detail: "O Lab baixa o yt-dlp oficial do GitHub, confere o SHA-256 publicado e guarda na pasta dele. Alternativa manual:",
      command: "winget install yt-dlp",
      action: (
        <div className="mt-2 flex flex-wrap items-center gap-3">
          <Button size="sm" variant="primary" loading={installing} onClick={() => void install()}>
            {installing ? "Instalando…" : "Instalar automaticamente"}
          </Button>
          {installError && <span role="alert" className="text-red-300">{installError}</span>}
        </div>
      ),
    });
  }
  if (issues.length === 0) return null;
  return (
    <div className="mx-8 mb-4 rounded-lg border border-amber-400/20 bg-amber-500/5 p-4">
      <div className="flex items-start justify-between gap-4">
        <p className="flex items-center gap-2 text-sm font-medium text-amber-200">
          <IconAlert />
          Falta preparar esta máquina
        </p>
        <Button size="sm" variant="ghost" loading={checking} onClick={onCheck}>Verificar de novo</Button>
      </div>
      <ul className="mt-3 space-y-3">
        {issues.map((issue) => (
          <li key={issue.title} className="text-xs">
            <p className="text-zinc-200">{issue.title}</p>
            <p className="mt-0.5 break-words text-zinc-500">{issue.detail}</p>
            {issue.action}
            <code className="mt-1.5 block rounded border border-white/5 bg-black/40 px-2 py-1.5 font-mono text-[11px] break-all text-zinc-300 select-all">{issue.command}</code>
          </li>
        ))}
      </ul>
      <p className="mt-3 text-[11px] text-zinc-500">Depois de instalar, feche e abra o Lab para ele enxergar o PATH novo.</p>
    </div>
  );
}

/** Banco de testes do script do jogo: vídeos, gabarito, resultados e percentuais. */
export default function ValidationView({ gameId, gameTitle, script, environment, checkingEnvironment, onCheckEnvironment }: ValidationViewProps) {
  const { dataset, error } = useValidationDataset(gameId);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [url, setUrl] = useState("");
  const [adding, setAdding] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    setSelectedId(null);
    setActionError(null);
  }, [gameId]);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 4000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const videos = dataset?.videos ?? [];
  const remaining = useRemainingTimes(videos);
  const summary = dataset?.resumo;
  const selected = videos.find((video) => video.id === selectedId) ?? null;
  const ready = Boolean(environment?.python.ok && environment.dependencias.ok);
  const canRun = script !== null && ready;
  const pending = videos.filter((video) => video.estado.fase === "pronto" && video.caminho !== null);
  const rerunnable = videos.filter(runnable);

  async function act(action: () => Promise<unknown>, success?: string) {
    setActionError(null);
    try {
      await action();
      if (success) setNotice(success);
    } catch (e) {
      setActionError(describe(e));
    }
  }

  async function onAddYoutube(event: FormEvent) {
    event.preventDefault();
    if (!url.trim()) return;
    setAdding(true);
    await act(async () => {
      await addYoutubeVideo(gameId, url.trim());
      setUrl("");
    }, "Download iniciado. O vídeo aparece na lista com o progresso.");
    setAdding(false);
  }

  async function onAddFiles() {
    const picked = await open({ multiple: true, title: "Escolha as gameplays", filters: [{ name: "Vídeos", extensions: ["mp4", "mkv", "webm", "mov", "avi"] }] });
    const paths = picked === null ? [] : Array.isArray(picked) ? picked : [picked];
    if (paths.length === 0) return;
    await act(() => addLocalVideos(gameId, paths), paths.length === 1 ? "1 vídeo adicionado." : `${paths.length} vídeos adicionados.`);
  }

  async function run(list: ValidationVideo[]) {
    await act(async () => {
      const queued = await runVideos(gameId, list.map((video) => video.id));
      setNotice(queued === 1 ? "1 vídeo na fila de análise." : `${queued} vídeos na fila de análise.`);
    });
  }

  const saveExpected = (video: ValidationVideo, expected: Expected) => act(() => setExpected(gameId, video.id, expected));
  const cancel = (video: ValidationVideo) => void act(() => cancelProcessing(gameId, video.id), "Cancelado.");
  const retry = (video: ValidationVideo) => void act(() => downloadAgain(gameId, video.id), "Download reiniciado.");

  return (
    <div className="relative flex min-w-0 flex-1 overflow-hidden">
      <section className="flex min-w-0 flex-1 flex-col">
        <header className="flex flex-wrap items-end justify-between gap-4 px-8 pt-7 pb-5">
          <div className="min-w-0">
            <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">Validação do script</p>
            <h1 className="mt-0.5 flex flex-wrap items-center gap-3 text-xl font-semibold text-zinc-100">
              {gameTitle}
              {script ? <Badge tone="info">script {script.id} {script.version}</Badge> : <Badge tone="warning">sem script</Badge>}
            </h1>
          </div>
          <div className="flex items-center gap-2">
            <Button disabled={!canRun || rerunnable.length === 0} onClick={() => void run(rerunnable)}>
              <IconRefresh />
              Rodar todos
            </Button>
            <Button variant="primary" disabled={!canRun || pending.length === 0} onClick={() => void run(pending)}>
              Rodar pendentes{pending.length > 0 && ` (${pending.length})`}
            </Button>
          </div>
        </header>

        {!script && (
          <p className="mx-8 mb-4 rounded-lg border border-white/5 bg-zinc-950/60 px-4 py-3 text-sm text-zinc-400">
            Este jogo ainda não tem script de validação registrado em <code className="font-mono text-zinc-300">shmupx_offline</code>. Dá para reunir os vídeos e o gabarito agora e rodar quando o script existir.
          </p>
        )}
        {environment && <EnvironmentNotice environment={environment} checking={checkingEnvironment} onCheck={onCheckEnvironment} />}

        <div className="grid grid-cols-1 gap-3 px-8 pb-5 md:grid-cols-3">
          <Metric
            label="Acerto"
            value={summary ? (percent(summary.acertos, summary.com_gabarito) ?? "—") : "—"}
            detail={summary ? `${summary.acertos} de ${summary.com_gabarito} com gabarito` : "—"}
            hint="Placar lido igual ao esperado."
          />
          <Metric
            label="Aceitação automática"
            value={summary ? (percent(summary.aceitos, summary.analisados) ?? "—") : "—"}
            detail={summary ? `${summary.aceitos} de ${summary.analisados} analisados` : "—"}
            hint="O Client aceitaria sem revisão."
          />
          <Metric
            label="Vídeos"
            value={summary?.total ?? "—"}
            detail={summary ? `${summary.analisados} analisados · ${pending.length} pendentes` : "—"}
            hint="Vídeos sem gabarito não entram no acerto."
          />
        </div>

        <form onSubmit={onAddYoutube} className="flex flex-wrap items-center gap-2 px-8 pb-4">
          <input
            type="url"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="Cole o link de uma gameplay no YouTube"
            aria-label="Link do YouTube"
            className="h-9 min-w-64 flex-1 rounded-md border border-white/10 bg-zinc-950 px-3 text-sm text-zinc-100 placeholder:text-zinc-600 outline-none focus:border-cyan-400/60 focus:ring-2 focus:ring-cyan-400/15"
          />
          <Button type="submit" loading={adding} disabled={!url.trim() || environment?.ytdlp.ok === false}>
            <IconPlus />
            Adicionar do YouTube
          </Button>
          <Button onClick={() => void onAddFiles()}>Adicionar arquivos</Button>
        </form>
        <p className="-mt-2 px-8 pb-3 text-[11px] text-zinc-600">Use gameplays suas ou de quem autorizou: os vídeos ficam só neste PC.</p>

        {(error || actionError) && (
          <p role="alert" className="mx-8 mb-4 flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
            <IconAlert className="mt-0.5 flex-shrink-0" />
            {actionError ?? error}
          </p>
        )}

        <div className="mx-8 mb-8 min-h-0 flex-1 overflow-auto rounded-lg border border-white/5 bg-zinc-950/40">
          {!dataset ? (
            <div aria-busy="true" className="space-y-px">
              {[0, 1, 2].map((row) => (
                <div key={row} className="flex gap-4 px-4 py-4">
                  <div className="h-3 w-64 animate-pulse rounded bg-white/5" />
                  <div className="ml-auto h-3 w-20 animate-pulse rounded bg-white/5" />
                </div>
              ))}
            </div>
          ) : videos.length === 0 ? (
            <div className="flex h-full min-h-56 flex-col items-center justify-center gap-2 px-6 py-14 text-center">
              <p className="text-sm font-medium text-zinc-200">Nenhum vídeo ainda</p>
              <p className="max-w-md text-xs text-zinc-500">
                Adicione gameplays do YouTube ou arquivos locais, informe o placar real de cada uma e rode o script para medir o acerto.
              </p>
            </div>
          ) : (
            <VideoTable
              videos={videos}
              selectedId={selectedId}
              canRun={canRun}
              remaining={remaining}
              onOpen={(video) => setSelectedId(video.id)}
              onRun={(video) => void run([video])}
              onCancel={cancel}
              onDownloadAgain={retry}
              onSaveExpected={saveExpected}
            />
          )}
        </div>
      </section>

      {selected && (
        <VideoPanel
          key={selected.id}
          video={selected}
          canRun={canRun}
          remaining={remaining.get(selected.id)}
          onClose={() => setSelectedId(null)}
          onRun={() => void run([selected])}
          onCancel={() => cancel(selected)}
          onDownloadAgain={() => retry(selected)}
          onSaveExpected={(expected) => saveExpected(selected, expected)}
          onRemove={async () => {
            await act(() => removeVideo(gameId, selected.id), "Vídeo removido.");
            setSelectedId(null);
          }}
        />
      )}

      {notice && (
        <div role="status" className="pointer-events-none fixed right-6 bottom-6 z-30 flex items-center gap-2 rounded-lg border border-emerald-400/20 bg-zinc-900 px-4 py-3 text-sm text-zinc-100 shadow-xl shadow-black/40">
          <IconCheck className="text-emerald-300" />
          {notice}
        </div>
      )}
    </div>
  );
}
