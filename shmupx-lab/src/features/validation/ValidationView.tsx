import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import { IconAlert, IconCheck, IconPaperclip, IconRefresh, IconYoutube } from "../../components/ui/Icons";
import {
  installYtDlp,
  cancelProcessing,
  downloadAgain,
  addLocalVideos,
  addYoutubeVideo,
  COOKIE_BROWSERS,
  defaultScript,
  isYoutubeBlock,
  newScriptVersion,
  setCookiesBrowser,
  percent,
  queuePosition,
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
import GenerateScriptDialog from "./GenerateScriptDialog";
import ApproveVersionDialog from "./ApproveVersionDialog";
import ScriptViewer from "./ScriptViewer";
import VideoPanel from "./VideoPanel";
import VideoTable from "./VideoTable";

interface ValidationViewProps {
  gameId: string;
  gameTitle: string;
  /** Versões do script do jogo, da mais nova para a mais antiga; vazio = o jogo ainda não tem script. */
  scripts: ScriptInfo[];
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
      command: "cd shmupx-lab\\validators; py -3.12 -m venv .venv; .\\.venv\\Scripts\\pip install -r requirements.txt",
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
  if (environment.ytdlp.ok && !environment.deno.ok) {
    issues.push({
      title: "Deno não instalado (o yt-dlp precisa dele para o YouTube)",
      detail: "Sem um runtime JavaScript, o yt-dlp extrai o YouTube de forma incompleta e o bloqueio “confirme que não é um robô” fica mais provável. Ele usa o Deno do PATH sozinho:",
      command: "winget install DenoLand.Deno",
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
export default function ValidationView({ gameId, gameTitle, scripts, environment, checkingEnvironment, onCheckEnvironment }: ValidationViewProps) {
  const { dataset, error } = useValidationDataset(gameId);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [url, setUrl] = useState("");
  const [adding, setAdding] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [showScript, setShowScript] = useState(false);
  const [showGenerate, setShowGenerate] = useState(false);
  const [creatingVersion, setCreatingVersion] = useState(false);
  const [showApprove, setShowApprove] = useState(false);
  // Versão escolhida; sem escolha (ou se ela sumir), vale a padrão: a mais nova aprovada.
  const [version, setVersion] = useState<string | null>(null);
  const script = scripts.find((item) => item.version === version) ?? defaultScript(scripts);

  useEffect(() => {
    setSelectedId(null);
    setActionError(null);
    setVersion(null);
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
  const incomplete = script?.missing ?? [];
  const canRun = script !== null && ready && incomplete.length === 0;
  const pending = videos.filter((video) => video.estado.fase === "pronto" && video.caminho !== null);
  const rerunnable = videos.filter(runnable);
  const blocked = videos.some((video) => video.estado.fase === "falhou" && isYoutubeBlock(video.estado.erro));
  const [savingCookies, setSavingCookies] = useState(false);

  async function changeCookies(browser: string | null) {
    setSavingCookies(true);
    await act(async () => {
      await setCookiesBrowser(browser);
      onCheckEnvironment();
    }, browser ? "Cookies do YouTube ativados para os próximos downloads." : "Downloads sem cookies do YouTube.");
    setSavingCookies(false);
  }

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
    if (!script) return;
    const scriptVersion = script.version;
    await act(async () => {
      const queued = await runVideos(gameId, scriptVersion, list.map((video) => video.id));
      setNotice(queued === 1 ? "1 vídeo na fila de análise." : `${queued} vídeos na fila de análise.`);
    });
  }

  async function createVersion() {
    if (!script) return;
    setCreatingVersion(true);
    await act(async () => {
      const created = await newScriptVersion(gameId, script.version);
      setVersion(created.versao);
      onCheckEnvironment();
      setNotice(`Versão ${created.versao} criada a partir da ${script.version} (rascunho).`);
    });
    setCreatingVersion(false);
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
              {script ? (
                <>
                  <select
                    value={script.version}
                    onChange={(event) => setVersion(event.target.value)}
                    aria-label="Versão do script"
                    title="Versão do script usada para rodar e ver"
                    className="h-7 rounded-md border border-white/10 bg-zinc-950 px-2 font-mono text-xs font-normal text-zinc-200 outline-none focus:border-cyan-400/60"
                  >
                    {scripts.map((item) => (
                      <option key={item.version} value={item.version}>
                        {item.version}{item.status === "draft" ? " · rascunho" : ""}
                      </option>
                    ))}
                  </select>
                  {script.status === "draft" ? <Badge tone="warning">rascunho</Badge> : <Badge tone="info">aprovado</Badge>}
                </>
              ) : (
                <Badge tone="warning">sem script</Badge>
              )}
            </h1>
          </div>
          <div className="flex items-center gap-2">
            {script?.status === "draft" && (
              <Button variant="success" disabled={incomplete.length > 0} onClick={() => setShowApprove(true)}
                title={incomplete.length > 0 ? `Faltam imagens do HUD na ${script.version}: complete antes de aprovar` : `Aprova a ${script.version}: o Client passa a validar as runs de ${gameTitle} com ela`}>
                <IconCheck />
                Aprovar versão
              </Button>
            )}
            {script ? (
              <Button loading={creatingVersion} onClick={() => void createVersion()} title={`Cria a próxima versão (rascunho) copiando a ${script.version}`}>
                Nova versão
              </Button>
            ) : (
              <Button onClick={() => setShowGenerate(true)} title="Cria o primeiro script deste jogo a partir do Earthion">
                Gerar script
              </Button>
            )}
            <Button variant="ghost" disabled={!script} onClick={() => setShowScript(true)} title={script ? "Ver o script Python que valida este jogo" : "Este jogo ainda não tem script"}>
              Ver script
            </Button>
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
            Este jogo ainda não tem script de validação em <code className="font-mono text-zinc-300">shmupx-lab/validators/games/{gameId}</code>. Dá para reunir os vídeos e o gabarito agora e rodar quando o script existir.
          </p>
        )}
        {script && incomplete.length > 0 && (
          <div className="mx-8 mb-4 rounded-lg border border-amber-400/20 bg-amber-500/5 p-4 text-xs">
            <p className="flex items-center gap-2 text-sm font-medium text-amber-200">
              <IconAlert />
              O script {script.version} ainda não pode rodar: faltam as imagens do HUD deste jogo
            </p>
            <ul className="mt-2 space-y-0.5 font-mono text-[11px] text-zinc-300">
              {incomplete.map((item) => <li key={item}>{item}</li>)}
            </ul>
            <p className="mt-2 text-zinc-400">
              Recorte-as seguindo o passo a passo em <strong>Ver script</strong> → <code className="font-mono">LEIA-ME.md</code> (pasta{" "}
              <code className="font-mono">validators/games/{gameId}/{script.version}/templates</code>) e clique em <strong>Verificar de novo</strong>.
            </p>
            <div className="mt-3">
              <Button size="sm" variant="ghost" loading={checkingEnvironment} onClick={onCheckEnvironment}>Verificar de novo</Button>
            </div>
          </div>
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
            <IconYoutube />
            Adicionar do YouTube
          </Button>
          <Button onClick={() => void onAddFiles()}>
            <IconPaperclip />
            Adicionar arquivos
          </Button>
        </form>
        <div className="-mt-2 flex flex-wrap items-center justify-between gap-x-6 gap-y-2 px-8 pb-3 text-[11px]">
          <p className="text-zinc-600">Use gameplays suas ou de quem autorizou: os vídeos ficam só neste PC.</p>
          <label className="flex items-center gap-2 text-zinc-400" title="O yt-dlp usa os cookies deste navegador; escolha um em que você esteja logado no YouTube. Contorna o bloqueio “confirme que não é um robô”.">
            Cookies do YouTube
            <select
              value={environment?.cookies_navegador ?? ""}
              disabled={!environment || savingCookies}
              onChange={(event) => void changeCookies(event.target.value || null)}
              className={`h-7 rounded-md border bg-zinc-950 px-2 text-xs text-zinc-200 outline-none focus:border-cyan-400/60 ${blocked && !environment?.cookies_navegador ? "border-amber-400/60" : "border-white/10"}`}
            >
              <option value="">nenhum</option>
              {COOKIE_BROWSERS.map((browser) => (
                <option key={browser.id} value={browser.id}>{browser.label}</option>
              ))}
            </select>
          </label>
        </div>
        {blocked && (
          <p role="status" className="mx-8 mb-4 flex gap-2 rounded-md border border-amber-400/20 bg-amber-500/5 px-4 py-3 text-xs leading-relaxed text-amber-100">
            <IconAlert className="mt-0.5 flex-shrink-0 text-amber-300" />
            <span>
              O YouTube bloqueou downloads deste PC (pediu login para confirmar que não é um robô). Em <strong>Cookies do YouTube</strong>, escolha um navegador em que você esteja logado no YouTube — no Windows, o Firefox costuma funcionar melhor que Chrome e Edge — e clique em <strong>Baixar de novo</strong>.
            </span>
          </p>
        )}

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
          queue={queuePosition(videos, selected)}
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

      {showScript && script && <ScriptViewer gameId={gameId} script={script} onClose={() => setShowScript(false)} />}
      {showApprove && script && (
        <ApproveVersionDialog
          gameId={gameId}
          gameTitle={gameTitle}
          version={script.version}
          summary={summary && summary.com_gabarito > 0 ? `${summary.acertos} de ${summary.com_gabarito} vídeos com gabarito acertados` : null}
          onClose={() => setShowApprove(false)}
          onApproved={() => {
            setVersion(script.version);
            onCheckEnvironment();
          }}
        />
      )}
      {showGenerate && (
        <GenerateScriptDialog
          gameId={gameId}
          gameTitle={gameTitle}
          onClose={() => setShowGenerate(false)}
          onGenerated={() => {
            setVersion(null);
            onCheckEnvironment();
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
