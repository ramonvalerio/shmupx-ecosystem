import { useEffect, useId, useRef, useState } from "react";
import type { GameRoom } from "../types/room";
import { useGameArt } from "../data/gameArt";
import { createPortal } from "react-dom";
import { setCabinetMuted, setCabinetVolume, useCabinetMuted, useCabinetVolume, useGameVideo } from "../data/gameVideo";
import { useDialogFocus } from "./useDialogFocus";
import { useI18n } from "../i18n";

/**
 * Monitor TATE (vertical) quando a arte do jogo é mais alta que larga: as artes de jogos de tela
 * vertical (Ketsui, DoDonPachi, Espgaluda…) são verticais; as de tela horizontal (Aero Fighters 2,
 * Darius Gaiden, Earthion…) são largas.
 */
function useTate(src: string) {
  const [tate, setTate] = useState(false);
  useEffect(() => {
    let alive = true;
    const image = new Image();
    image.onload = () => { if (alive) setTate(image.naturalHeight > image.naturalWidth * 1.05); };
    image.src = src;
    return () => { alive = false; };
  }, [src]);
  return tate;
}

/**
 * Tubo CRT (moldura) e tela: 4:3 deitado (YOKO) ocupa a largura do monitor e a máscara preta fica
 * em cima e embaixo; 3:4 em pé (TATE) ocupa a altura e a máscara fica nas laterais.
 */
const BEZEL = {
  yoko: { x: 52, y: 44, width: 216, height: 162, rx: 12 },
  tate: { x: 96, y: 40, width: 128, height: 170, rx: 12 },
};
const SCREEN = {
  yoko: { x: 58, y: 50, width: 204, height: 153, rx: 6 },
  tate: { x: 102, y: 47, width: 116, height: 155, rx: 6 },
};

function SpeakerIcon({ silent }: { silent: boolean }) {
  return (
    <svg viewBox="0 0 24 24" width="12" height="12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 9h4l5-4v14l-5-4H4z" fill="currentColor" />
      {silent ? <path d="M17 9l5 6M22 9l-5 6" /> : <path d="M17 8.5a5 5 0 0 1 0 7M19.5 6a8.5 8.5 0 0 1 0 12" />}
    </svg>
  );
}

/**
 * Trailer na tela do gabinete: toca só com o gabinete visível, o Client em foco e a modal fechada
 * (ao abrir o jogo a janela perde o foco e o vídeo pausa, sem som por cima do jogo). Com som; na
 * barra (ao passar o mouse): silenciar, volume e expandir. Duplo clique também expande.
 */
function CabinetVideo({ src, paused, resume, onFail, onExpand }: { src: string; paused: boolean; resume: { time: number } | null; onFail: () => void; onExpand: (time: number) => void }) {
  const { t } = useI18n();
  const ref = useRef<HTMLVideoElement>(null);
  const muted = useCabinetMuted();
  const volume = useCabinetVolume();
  const [blocked, setBlocked] = useState(false);
  const silent = muted || blocked;
  const pausedRef = useRef(paused);
  pausedRef.current = paused;
  const updateRef = useRef<() => void>(() => undefined);

  useEffect(() => {
    const video = ref.current;
    if (!video) return;
    video.muted = silent;
    video.volume = volume;
  }, [silent, volume]);

  useEffect(() => {
    const video = ref.current;
    if (!video) return;
    let visible = false;
    let focused = document.hasFocus();
    const update = () => {
      if (!visible || !focused || pausedRef.current) { video.pause(); return; }
      video.play().catch(() => {
        // Som automático bloqueado: segue mudo (o alto-falante mostra e religa).
        if (!video.muted) { video.muted = true; setBlocked(true); void video.play().catch(() => undefined); }
      });
    };
    updateRef.current = update;
    const observer = new IntersectionObserver(([entry]) => { visible = entry.isIntersecting; update(); });
    // Dentro do foreignObject o Chromium não informa a visibilidade do vídeo: observa o SVG do gabinete.
    observer.observe(video.closest("svg") ?? video);
    const onFocus = () => { focused = true; update(); };
    const onBlur = () => { focused = false; update(); };
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    return () => {
      observer.disconnect();
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      video.pause();
    };
  }, [src]);
  // Ao fechar a modal, continua de onde o vídeo grande parou.
  useEffect(() => {
    const video = ref.current;
    if (!video || !resume) return;
    video.currentTime = resume.time;
  }, [resume]);
  useEffect(() => updateRef.current(), [paused]);

  const expand = () => onExpand(ref.current?.currentTime ?? 0);
  return (
    <div className="cabinet-video" onDoubleClick={expand} title={t("Duplo clique para ver o vídeo grande")}>
      {/* O trailer inteiro, recomeçando ao terminar. */}
      <video ref={ref} src={src} loop playsInline preload="auto" onError={onFail} />
      <div className="cabinet-controls" onDoubleClick={(event) => event.stopPropagation()}>
        <button type="button" className="cabinet-control" aria-label={t(silent ? "Ligar o som do vídeo" : "Silenciar o vídeo")} title={t(silent ? "Ligar o som do vídeo" : "Silenciar o vídeo")}
          onClick={() => { if (blocked) { setBlocked(false); setCabinetMuted(false); } else setCabinetMuted(!muted); }}>
          <SpeakerIcon silent={silent} />
        </button>
        <input className="cabinet-volume" type="range" min={0} max={1} step={0.05} value={silent ? 0 : volume} aria-label={t("Volume do vídeo")} title={t("Volume do vídeo")}
          onChange={(event) => { const next = Number(event.target.value); setCabinetVolume(next); if (next > 0 && silent) { setBlocked(false); setCabinetMuted(false); } }} />
        <button type="button" className="cabinet-control cabinet-expand" aria-label={t("Ver o vídeo grande")} title={t("Ver o vídeo grande")} onClick={expand}>
          <svg viewBox="0 0 24 24" width="12" height="12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
            <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
          </svg>
        </button>
      </div>
    </div>
  );
}

/**
 * Vídeo do jogo grande, numa janela no centro: controles do player (volume, avanço), com o volume
 * e o som do gabinete. Fecha no X, no Esc ou clicando fora.
 */
function VideoModal({ title, src, start, onClose }: { title: string; src: string; start: number; onClose: (time: number) => void }) {
  const { t } = useI18n();
  const videoRef = useRef<HTMLVideoElement | null>(null);
  // Fecha informando onde o vídeo parou, para o gabinete continuar dali.
  const close = () => onClose(videoRef.current?.currentTime ?? start);
  const ref = useDialogFocus(true, close);
  const muted = useCabinetMuted();
  const volume = useCabinetVolume();
  return createPortal(
    <div className="controller-dialog-backdrop video-modal-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) close(); }}>
      <div ref={ref} role="dialog" aria-modal="true" aria-label={t("Vídeo de {game}", { game: title })} tabIndex={-1} className="video-modal">
        <header>
          <strong>{title}</strong>
          <button type="button" className="video-modal-close" aria-label={t("Fechar")} onClick={close}>✕</button>
        </header>
        <video src={src} controls autoPlay loop playsInline
          ref={(video) => { videoRef.current = video; if (video && !video.dataset.ready) { video.dataset.ready = "1"; video.currentTime = start; video.volume = volume; video.muted = muted; } }}
          onVolumeChange={(event) => { setCabinetVolume(event.currentTarget.volume); setCabinetMuted(event.currentTarget.muted); }} />
      </div>
    </div>,
    document.body,
  );
}

/**
 * Gabinete arcade com a capa do jogo na tela. Com `video` (Minhas coleções), a tela mostra o
 * trailer do jogo quando houver; na sala, fica a imagem.
 */
export function ArcadeCabinet({ room, className = "", video: withVideo = false }: { room: GameRoom; className?: string; video?: boolean }) {
  const art = useGameArt(room);
  const video = useGameVideo(room, withVideo);
  const [videoFailed, setVideoFailed] = useState(false);
  const [modalStart, setModalStart] = useState<number | null>(null);
  const [resume, setResume] = useState<{ time: number } | null>(null);
  useEffect(() => { setVideoFailed(false); setModalStart(null); setResume(null); }, [video]);
  const showVideo = Boolean(video) && !videoFailed;
  const artTate = useTate(art.src);
  // Trailers são largos (16:9): com vídeo, a tela fica deitada.
  const tate = artTate && !showVideo;
  const screen = tate ? SCREEN.tate : SCREEN.yoko;
  const bezel = tate ? BEZEL.tate : BEZEL.yoko;
  // Ids únicos: a Coleção e a sala podem mostrar dois gabinetes ao mesmo tempo.
  const uid = useId().replace(/:/g, "");
  const id = (name: string) => `${name}-${uid}`;
  return (
    <>
    {showVideo && modalStart !== null && <VideoModal title={room.title} src={video!} start={modalStart} onClose={(time) => { setResume({ time }); setModalStart(null); }} />}
    <svg className={`arcade-cabinet ${tate ? "is-tate" : ""} ${className}`} viewBox="22 0 276 327" role="img" aria-label={room.title}>
      {/* Inspirado no Sega Astro City: corpo claro de cantos redondos, marquise iluminada (sem texto),
          monitor fundo em moldura escura, painel preto largo e porta da ficha embaixo. */}
      <defs>
        <clipPath id={id("screen")}><rect {...screen} /></clipPath>
        <linearGradient id={id("body")} x1="0" y1="0" x2="1" y2="0"><stop offset="0" stopColor="#b9c0c6" /><stop offset=".18" stopColor="#e9ecef" /><stop offset=".82" stopColor="#e9ecef" /><stop offset="1" stopColor="#b9c0c6" /></linearGradient>
        <linearGradient id={id("marquee")} x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#123038" /><stop offset="1" stopColor="#081418" /></linearGradient>
        <linearGradient id={id("glass")} x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#ffffff" stopOpacity=".1" /><stop offset=".45" stopColor="#ffffff" stopOpacity="0" /></linearGradient>
      </defs>
      {/* Corpo de cima: topo bem arredondado */}
      <path d="M66 4 H254 Q278 4 279 30 L281 218 H39 L41 30 Q42 4 66 4 Z" fill={`url(#${id("body")})`} stroke="#7d878e" strokeWidth="1.5" />
      {/* Marquise iluminada com a logo do ShmupX (fundo escuro para a logo ciano e vermelha ler bem) */}
      <rect x="58" y="12" width="204" height="22" rx="8" fill={`url(#${id("marquee")})`} stroke="#57fcff" strokeWidth="1.2" />
      <rect x="60" y="13.5" width="200" height="19" rx="7" fill="none" stroke="#57fcff" strokeOpacity=".25" strokeWidth="1" />
      <image href="/img/logo.png" x="124" y="14.5" width="72" height="17" preserveAspectRatio="xMidYMid meet" aria-hidden="true" />
      {/* Moldura escura e funda do monitor (a máscara em volta do tubo) */}
      <path d="M50 42 Q50 38 54 38 H266 Q270 38 270 42 L272 212 H48 Z" fill="#14181b" />
      {/* Vão do tubo: deitado (YOKO) ou em pé (TATE) */}
      <rect {...bezel} fill="#050708" stroke="#2a3136" strokeWidth="2" />
      <rect {...screen} fill="#000" />
      {showVideo
        // Sem clip-path no foreignObject: o Chromium deixa de entregar o mouse aos controles. O canto
        // arredondado da tela vem do CSS (.cabinet-video).
        ? <foreignObject {...screen}><CabinetVideo src={video!} paused={modalStart !== null} resume={resume} onFail={() => setVideoFailed(true)} onExpand={setModalStart} /></foreignObject>
        : <image className={art.grayscale ? "is-not-owned" : ""} href={art.src} {...screen} preserveAspectRatio="xMidYMid meet" clipPath={`url(#${id("screen")})`} onError={art.onError} />}
      <rect {...screen} fill={`url(#${id("glass")})`} pointerEvents="none" />
      {/* Painel, corpo de baixo e portas: abaixo do monitor */}
      <g transform="translate(0 36)">
      {/* Painel de controle preto, largo e projetado à frente */}
      <path d="M34 182 H286 Q294 182 294 191 L295 207 Q295 216 286 216 H34 Q25 216 25 207 L26 191 Q26 182 34 182 Z" fill="#16191c" stroke="#7d878e" strokeWidth="1.5" />
      <path d="M30 186 H290" stroke="#ffffff" strokeOpacity=".12" strokeWidth="1" />
      {/* Layout do painel do Astro City: joystick à esquerda, START pequeno em cima, entre os dois, e
          os 3 botões em arco (o 1º mais baixo, o 2º e o 3º mais altos) sobre a faixa cinza. */}
      <polyline points="163,204 173,197.5 195,197.5" fill="none" stroke="#5c656b" strokeWidth="15" strokeLinecap="round" strokeLinejoin="round" />
      <polyline points="163,204 173,197.5 195,197.5" fill="none" stroke="#3a4045" strokeWidth="13.4" strokeLinecap="round" strokeLinejoin="round" />
      <ellipse cx="123" cy="200" rx="7" ry="5" fill="#0b0d0f" />
      <circle cx="122" cy="196" r="7" fill="#fd4851" />
      <circle cx="120" cy="193.5" r="2" fill="#ffffff" fillOpacity=".35" />
      {/* Crédito (START): botão redondo azul, como os vermelhos */}
      <circle cx="145.5" cy="188" r="3.6" fill="#2f7bff" />
      <circle cx="144.6" cy="187" r="1" fill="#ffffff" fillOpacity=".35" />
      <circle cx="163" cy="204" r="4.6" fill="#fd4851" />
      <circle cx="179" cy="197.5" r="4.6" fill="#fd4851" />
      <circle cx="195" cy="197.5" r="4.6" fill="#fd4851" />
      {/* Corpo de baixo claro, reto: mesma largura do corpo de cima, cantos de baixo levemente arredondados */}
      <path d="M39 216 H281 V281 Q281 288 274 288 H46 Q39 288 39 281 Z" fill={`url(#${id("body")})`} stroke="#7d878e" strokeWidth="1.5" />
      {/* Esquerda: tampa de manutenção (só o recorte e a fechadura) */}
      <rect x="56" y="224" width="130" height="54" rx="4" fill="none" stroke="#a3abb1" strokeWidth="1.2" />
      <circle cx="176" cy="251" r="2.6" fill="#8a939a" />
      <rect x="175.4" y="250" width="1.2" height="3.4" rx=".5" fill="#4b5359" />
      {/* Direita: porta da ficha, branca como a tampa da esquerda (só o recorte), com a entrada de
          moeda e o retorno de moedas */}
      <rect x="198" y="226" width="66" height="50" rx="3" fill="none" stroke="#8a939a" strokeWidth="1.2" />
      <rect x="221" y="234" width="20" height="20" rx="2" fill="#9aa3a9" stroke="#7d878e" strokeWidth="1" />
      <rect x="230" y="238" width="2" height="12" rx="1" fill="#2b3035" />
      <rect x="219" y="261" width="24" height="7" rx="1.5" fill="#8a939a" stroke="#7d878e" strokeWidth=".8" />
      {/* Rodapé: linha horizontal cinza de lado a lado, com margem até a base */}
      <line x1="39.75" y1="281" x2="280.25" y2="281" stroke="#8a939a" strokeWidth="1.5" />
      </g>
    </svg>
    </>
  );
}
