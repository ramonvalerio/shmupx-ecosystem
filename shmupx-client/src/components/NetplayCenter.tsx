import { useEffect, useRef, useState } from "react";
import { doc, onSnapshot } from "firebase/firestore";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../i18n";
import { db } from "../auth/firebase";
import type { GameRoom } from "../types/room";
import { useEmulatorLibrary } from "../data/emulatorLibrary";
import {
  createInvite, finishSession, INVITE_TTL_MS, launchNetplay, prepareSide, READY_TTL_MS, setNetplayPartner, updateSession, watchSessions,
  type NetplayEngineEvent, type NetplaySession, type NetplaySide,
} from "../data/netplay";
import { useDialogFocus } from "./useDialogFocus";
import DialogClose from "./DialogClose";

const INVITE_EVENT = "shmupx:netplay-invite";

/** Clique direito › Convidar para jogar: o NetplayCenter (montado no App) cuida do resto. */
export function inviteToPlay(guest: { uid: string; name: string }, room: GameRoom) {
  window.dispatchEvent(new CustomEvent(INVITE_EVENT, { detail: { guest, room } }));
}

type Translate = ReturnType<typeof useI18n>["t"];

/** Dupla em andamento neste Client (uma por vez). */
interface Flow {
  sessionId: string;
  role: "host" | "guest";
  partner: string;
  room: GameRoom;
  phase: "waiting" | "preparing" | "launching" | "connecting" | "playing";
  side?: NetplaySide;
  createdAt: number;
  rtt?: number;
  unstable?: boolean;
}

interface Notice {
  title: string;
  text: string;
  steps: string[];
}

/** O que dizer ao jogador quando a dupla acaba ou não começa: sem termos de rede (o motivo técnico fica no log). */
function noticeFor(reason: string, partner: string, game: string, t: Translate): Notice | null {
  switch (reason) {
    case "declined":
      return { title: t("{name} recusou o convite", { name: partner }), text: t("Tente convidar de novo mais tarde."), steps: [] };
    case "expired":
      return { title: t("{name} não respondeu", { name: partner }), text: t("O convite para jogar {game} em dupla expirou.", { game }), steps: [] };
    case "busy":
      return { title: t("{name} já está em outra partida", { name: partner }), text: t("Espere a partida terminar e convide de novo."), steps: [] };
    case "no_rom":
      return { title: t("{name} não tem {game}", { name: partner, game }), text: t("Para jogar em dupla, os dois precisam ter o jogo no ShmupX Emulator."), steps: [] };
    case "partner_left":
      return { title: t("{name} saiu da partida", { name: partner }), text: t("O treino em dupla terminou."), steps: [] };
    case "timeout":
      return {
        title: t("A conexão com {name} caiu", { name: partner }),
        text: t("O computador de {name} parou de responder.", { name: partner }),
        steps: [t("Confiram se a internet dos dois está funcionando."), t("Depois, convide de novo.")],
      };
    case "connect_failed":
    case "ready_timeout":
      return {
        title: t("Não deu para conectar com {name}", { name: partner }),
        text: t("Os dois computadores não conseguiram se encontrar pela internet."),
        steps: [
          t("Confiram se a internet dos dois está funcionando e tentem de novo."),
          t("Se o Windows perguntar se o ShmupX Emulator pode usar a rede, clique em Permitir."),
          t("Se continuar, desligue e ligue o aparelho da internet (roteador) e tente de novo."),
        ],
      };
    case "version_mismatch":
      return {
        title: t("Vocês estão com versões diferentes do ShmupX"),
        text: t("Para jogar em dupla, os dois precisam estar com a mesma versão."),
        steps: [t("Os dois atualizam o ShmupX Client para a versão mais nova."), t("Depois, convide de novo.")],
      };
    case "rom_mismatch":
    case "state_mismatch":
      return {
        title: t("O {game} de vocês é diferente", { game }),
        text: t("Os dois precisam ter exatamente o mesmo ROM do jogo."),
        steps: [t("Confiram se os dois usam o mesmo arquivo de ROM de {game}.", { game })],
      };
    case "needs_update":
      return {
        title: t("Atualize o ShmupX Client"),
        text: t("Esta versão do ShmupX Emulator ainda não tem o treino em dupla."),
        steps: [t("Atualize o ShmupX Client para a versão mais nova e tente de novo.")],
      };
    case "launch_failed":
      return {
        title: t("O jogo não abriu"),
        text: t("Algo impediu {game} de abrir agora.", { game }),
        steps: [t("Confira se o jogo abre no Play Training."), t("Depois, convide de novo.")],
      };
    case "local_exit":
    case "closed":
      return null;
    default:
      return { title: t("O treino em dupla terminou"), text: t("A partida com {name} foi encerrada.", { name: partner }), steps: [] };
  }
}

/**
 * Treino em dupla online, para o Client todo: convites recebidos (modal Aceitar/Recusar), o convite
 * enviado esperando resposta, a abertura do motor nos dois PCs e os avisos da conexão.
 */
export default function NetplayCenter({ me, rooms }: { me: { uid: string; name: string }; rooms: GameRoom[] }) {
  const { t } = useI18n();
  const emulator = useEmulatorLibrary();
  const [incoming, setIncoming] = useState<NetplaySession[]>([]);
  const [flow, setFlowState] = useState<Flow | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const flowRef = useRef<Flow | null>(null);
  const roomsRef = useRef(rooms);
  roomsRef.current = rooms;
  const ownsRef = useRef(emulator.owns);
  ownsRef.current = emulator.owns;
  const handled = useRef(new Set<string>());
  // Com o jogo aberto, a lista de jogadores da sala mostra "Jogando X com <parceiro>".
  const partnerGame = flow && (flow.phase === "connecting" || flow.phase === "playing") ? flow.room.id : null;
  const partnerName = partnerGame ? flow!.partner : null;
  useEffect(() => {
    setNetplayPartner(partnerGame && partnerName ? { gameId: partnerGame, name: partnerName } : null);
  }, [partnerGame, partnerName]);
  useEffect(() => () => setNetplayPartner(null), []);

  function setFlow(next: Flow | null | ((current: Flow | null) => Flow | null)) {
    const value = typeof next === "function" ? next(flowRef.current) : next;
    flowRef.current = value;
    setFlowState(value);
  }

  function end(reason: string, status: "declined" | "canceled" | "ended" | "failed" = "ended") {
    const current = flowRef.current;
    if (!current) return;
    setFlow(null);
    void finishSession(current.sessionId, status, reason);
    const text = noticeFor(reason, current.partner, current.room.title, t);
    if (text) setNotice(text);
  }

  // Relógio para os prazos do convite.
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  // Convites recebidos (as sessões ativas em que este jogador é o convidado).
  useEffect(() => {
    if (!me.uid) return;
    return watchSessions(me.uid, (sessions) => setIncoming(sessions.filter((s) => s.guestUid === me.uid && s.status === "invited")));
  }, [me.uid]);

  // Ocupado em outra dupla: recusa sozinho os convites novos.
  useEffect(() => {
    if (!flowRef.current) return;
    for (const session of incoming)
      if (session.id !== flowRef.current.sessionId && !handled.current.has(session.id)) {
        handled.current.add(session.id);
        void finishSession(session.id, "declined", "busy");
      }
  }, [incoming]);

  // A sessão da dupla em andamento: respostas do parceiro e a vez de abrir o motor.
  const sessionId = flow?.sessionId ?? null;
  useEffect(() => {
    if (!sessionId) return;
    return onSnapshot(doc(db, "netplaySessions", sessionId), (snapshot) => {
      const current = flowRef.current;
      if (!current || current.sessionId !== sessionId) return;
      const data = snapshot.data();
      if (!data) { setFlow(null); return; }
      const status = data.status as string;
      const reason = typeof data.reason === "string" ? data.reason : status;
      if (!data.active) {
        // O parceiro encerrou: recusou, cancelou ou a partida dele terminou.
        if (current.phase === "playing" || current.phase === "connecting") return; // o motor avisa pelo próprio evento
        setFlow(null);
        const text = noticeFor(status === "declined" ? (reason === "declined" ? "declined" : reason) : reason, current.partner, current.room.title, t);
        if (text) setNotice(text);
        return;
      }
      if (current.role === "host" && status === "accepted" && current.phase === "waiting" && data.guest) {
        void hostStart(current, data.guest as NetplaySide, data.key as string);
      }
      if (current.role === "guest" && status === "ready" && current.phase === "preparing" && data.host && current.side) {
        void guestStart(current, current.side, data.host as NetplaySide, data.key as string);
      }
    }, (error) => console.warn("dupla: falha ao ouvir a sessão:", error));
  }, [sessionId]);

  // Prazos: convite sem resposta e anfitrião que não abriu o motor.
  useEffect(() => {
    const current = flowRef.current;
    if (!current) return;
    if (current.role === "host" && current.phase === "waiting" && now - current.createdAt > INVITE_TTL_MS) end("expired", "canceled");
    if (current.role === "guest" && current.phase === "preparing" && now - current.createdAt > READY_TTL_MS) end("ready_timeout", "failed");
  }, [now]);

  // Clique direito › Convidar para jogar.
  useEffect(() => {
    const onInvite = (event: Event) => {
      const { guest, room } = (event as CustomEvent<{ guest: { uid: string; name: string }; room: GameRoom }>).detail;
      if (flowRef.current || !me.uid) return;
      void (async () => {
        try {
          const id = await createInvite(me, guest, room);
          setFlow({ sessionId: id, role: "host", partner: guest.name, room, phase: "waiting", createdAt: Date.now() });
        } catch (error) {
          console.warn("dupla: falha ao criar o convite:", error);
          setNotice({ title: t("O convite não foi enviado"), text: t("Confira a internet e tente de novo."), steps: [] });
        }
      })();
    };
    window.addEventListener(INVITE_EVENT, onInvite);
    return () => window.removeEventListener(INVITE_EVENT, onInvite);
  }, [me.uid, me.name]);

  // Eventos do motor na dupla: conexão, ping e o fim da partida.
  useEffect(() => {
    const stopEvents = listen<NetplayEngineEvent>("dupla://evento", (event) => {
      const current = flowRef.current;
      if (!current) return;
      const e = event.payload.evento;
      switch (e.event) {
        case "connecting":
          setFlow({ ...current, phase: "connecting" });
          break;
        case "connected":
        case "running":
          setFlow({ ...current, phase: "playing", rtt: typeof e.rtt === "number" ? e.rtt : current.rtt, unstable: false });
          break;
        case "stats":
          if (typeof e.rtt === "number") setFlow({ ...current, rtt: e.rtt });
          break;
        case "unstable":
          setFlow({ ...current, unstable: true });
          break;
        case "stable":
          setFlow({ ...current, unstable: false });
          break;
        case "ended":
          end(typeof e.reason === "string" ? e.reason : "ended");
          break;
      }
    });
    // O motor fechou sem avisar o fim (janela fechada, falha): a sessão acaba.
    const stopExit = listen<{ pid: number }>("treino://encerrado", () => {
      const current = flowRef.current;
      if (current && (current.phase === "connecting" || current.phase === "playing" || current.phase === "launching")) end("closed");
    });
    return () => {
      void stopEvents.then((stop) => stop());
      void stopExit.then((stop) => stop());
    };
  }, []);

  async function hostStart(current: Flow, guest: NetplaySide, key: string) {
    setFlow({ ...current, phase: "preparing" });
    try {
      const side = await prepareSide();
      await updateSession(current.sessionId, { status: "ready", host: { port: side.port, candidates: side.candidates } });
      setFlow((f) => f && { ...f, phase: "launching", side });
      await launchNetplay(current.room, "host", side, guest, key);
    } catch (error) {
      console.warn("dupla: o anfitrião não abriu o motor:", error);
      end(String(error).includes("treino em dupla") ? "needs_update" : "launch_failed", "failed");
    }
  }

  async function guestStart(current: Flow, mine: NetplaySide, host: NetplaySide, key: string) {
    setFlow({ ...current, phase: "launching" });
    try {
      await launchNetplay(current.room, "guest", mine, host, key);
    } catch (error) {
      console.warn("dupla: o convidado não abriu o motor:", error);
      end(String(error).includes("treino em dupla") ? "needs_update" : "launch_failed", "failed");
    }
  }

  async function accept(session: NetplaySession) {
    handled.current.add(session.id);
    const room = roomsRef.current.find((r) => r.id === session.gameId);
    if (!room || !ownsRef.current(room)) {
      void finishSession(session.id, "failed", "no_rom");
      setNotice({ title: t("Você não tem {game}", { game: session.title }), text: t("Para jogar em dupla, importe o jogo no ShmupX Emulator em Importar jogos."), steps: [] });
      return;
    }
    const next: Flow = { sessionId: session.id, role: "guest", partner: session.hostName, room, phase: "preparing", createdAt: Date.now() };
    setFlow(next);
    try {
      const side = await prepareSide();
      setFlow((f) => f && { ...f, side });
      await updateSession(session.id, { status: "accepted", guest: { port: side.port, candidates: side.candidates } });
    } catch (error) {
      console.warn("dupla: falha ao aceitar:", error);
      end("launch_failed", "failed");
    }
  }

  function decline(session: NetplaySession) {
    handled.current.add(session.id);
    void finishSession(session.id, "declined", "declined");
  }

  function cancel() {
    end("canceled", "canceled");
  }

  const invite = !flow ? incoming.find((s) => !handled.current.has(s.id) && now - s.createdAt < INVITE_TTL_MS) ?? null : null;

  return <>
    {invite && <InviteDialog session={invite} onAccept={() => void accept(invite)} onDecline={() => decline(invite)} />}
    {flow && flow.role === "host" && flow.phase === "waiting" && <WaitingDialog flow={flow} secondsLeft={Math.max(0, Math.ceil((INVITE_TTL_MS - (now - flow.createdAt)) / 1000))} onCancel={cancel} />}
    {flow && flow.phase !== "waiting" && <NetplayStatus flow={flow} />}
    {notice && <NoticeDialog notice={notice} onClose={() => setNotice(null)} />}
  </>;
}

function InviteDialog({ session, onAccept, onDecline }: { session: NetplaySession; onAccept: () => void; onDecline: () => void }) {
  const { t } = useI18n();
  const ref = useDialogFocus(true, onDecline);
  return (
    <div className="controller-dialog-backdrop">
      <div ref={ref} role="alertdialog" aria-modal="true" aria-labelledby="netplay-invite-title" aria-describedby="netplay-invite-text" tabIndex={-1} className="controller-dialog close-room-dialog netplay-dialog">
        <header>
          <div>
            <h2 id="netplay-invite-title">{t("{name} convidou você para jogar", { name: session.hostName })}</h2>
            <p id="netplay-invite-text">{t("{game} em dupla, online, no ShmupX Emulator. É treino: não vale ranking nem gasta ficha. {name} é o jogador 1 e você, o jogador 2.", { game: session.title, name: session.hostName })}</p>
          </div>
          <DialogClose onClick={onDecline} label={t("Recusar")} />
        </header>
        <footer>
          <button type="button" className="secondary-action" onClick={onDecline}>{t("Recusar")}</button>
          <button type="button" className="primary-action" onClick={onAccept}>{t("Aceitar e jogar")}</button>
        </footer>
      </div>
    </div>
  );
}

function WaitingDialog({ flow, secondsLeft, onCancel }: { flow: Flow; secondsLeft: number; onCancel: () => void }) {
  const { t } = useI18n();
  const ref = useDialogFocus(true, onCancel);
  return (
    <div className="controller-dialog-backdrop">
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby="netplay-wait-title" tabIndex={-1} className="controller-dialog close-room-dialog netplay-dialog">
        <header>
          <div>
            <h2 id="netplay-wait-title">{t("Convite enviado para {name}", { name: flow.partner })}</h2>
            <p>{t("Esperando {name} aceitar para jogar {game} em dupla ({seconds} s).", { name: flow.partner, game: flow.room.title, seconds: String(secondsLeft) })}</p>
          </div>
        </header>
        <footer>
          <span />
          <button type="button" className="secondary-action" onClick={onCancel}>{t("Cancelar convite")}</button>
        </footer>
      </div>
    </div>
  );
}

/** Aviso discreto no canto: abrindo, conectando, em dupla (com o ping) ou conexão instável. */
function NetplayStatus({ flow }: { flow: Flow }) {
  const { t } = useI18n();
  const [hidden, setHidden] = useState(false);
  useEffect(() => {
    setHidden(false);
    if (flow.phase !== "playing" || flow.unstable) return;
    const timer = window.setTimeout(() => setHidden(true), 6000);
    return () => window.clearTimeout(timer);
  }, [flow.phase, flow.unstable]);
  if (hidden) return null;
  const text = flow.unstable
    ? t("Conexão instável com {name}…", { name: flow.partner })
    : flow.phase === "playing"
    ? t("Em dupla com {name}", { name: flow.partner }) + (typeof flow.rtt === "number" ? ` · ${flow.rtt} ms` : "")
    : flow.phase === "connecting"
    ? t("Conectando com {name}…", { name: flow.partner })
    : t("Abrindo {game} em dupla com {name}…", { game: flow.room.title, name: flow.partner });
  return <div className={`netplay-status ${flow.unstable ? "is-unstable" : ""}`} role="status">{text}</div>;
}

function NoticeDialog({ notice, onClose }: { notice: Notice; onClose: () => void }) {
  const { t } = useI18n();
  const ref = useDialogFocus(true, onClose);
  return (
    <div className="controller-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={ref} role="alertdialog" aria-modal="true" aria-labelledby="netplay-notice-title" aria-describedby="netplay-notice-text" tabIndex={-1} className="controller-dialog close-room-dialog launch-problem-dialog">
        <header>
          <div>
            <h2 id="netplay-notice-title">{notice.title}</h2>
            <p id="netplay-notice-text">{notice.text}</p>
          </div>
          <DialogClose onClick={onClose} />
        </header>
        {notice.steps.length > 0 && <ol className="launch-problem-steps">{notice.steps.map((step) => <li key={step}>{step}</li>)}</ol>}
        <footer>
          <span />
          <button type="button" className="primary-action" onClick={onClose}>{t("Entendi")}</button>
        </footer>
      </div>
    </div>
  );
}
