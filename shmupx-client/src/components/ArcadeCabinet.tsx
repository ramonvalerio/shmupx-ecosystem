import { useEffect, useId, useState } from "react";
import type { GameRoom } from "../types/room";
import { useGameArt } from "../data/gameArt";

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

/** Gabinete arcade com a capa do jogo na tela. */
export function ArcadeCabinet({ room, className = "" }: { room: GameRoom; className?: string }) {
  const art = useGameArt(room);
  const tate = useTate(art.src);
  const screen = tate ? SCREEN.tate : SCREEN.yoko;
  const bezel = tate ? BEZEL.tate : BEZEL.yoko;
  // Ids únicos: a Coleção e a sala podem mostrar dois gabinetes ao mesmo tempo.
  const uid = useId().replace(/:/g, "");
  const id = (name: string) => `${name}-${uid}`;
  return (
    <svg className={`arcade-cabinet ${tate ? "is-tate" : ""} ${className}`} viewBox="22 0 276 336" role="img" aria-label={room.title}>
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
      <image className={art.grayscale ? "is-not-owned" : ""} href={art.src} {...screen} preserveAspectRatio="xMidYMid meet" clipPath={`url(#${id("screen")})`} onError={art.onError} />
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
      <rect x="139" y="185" width="13" height="6" rx="3" fill="#2f7bff" />
      <circle cx="163" cy="204" r="4.6" fill="#fd4851" />
      <circle cx="179" cy="197.5" r="4.6" fill="#fd4851" />
      <circle cx="195" cy="197.5" r="4.6" fill="#fd4851" />
      {/* Corpo de baixo claro, reto: mesma largura do corpo de cima, laterais verticais até o chão */}
      <path d="M39 216 H281 V288 H39 Z" fill={`url(#${id("body")})`} stroke="#7d878e" strokeWidth="1.5" />
      {/* Esquerda: tampa de manutenção (só o recorte e a fechadura) */}
      <rect x="70" y="224" width="92" height="54" rx="4" fill="none" stroke="#a3abb1" strokeWidth="1.2" />
      <circle cx="152" cy="251" r="2.6" fill="#8a939a" />
      <rect x="151.4" y="250" width="1.2" height="3.4" rx=".5" fill="#4b5359" />
      {/* Direita: porta da ficha cinza, com duas entradas quadradas e o retorno de moedas */}
      <rect x="178" y="226" width="66" height="50" rx="3" fill="#c3c9ce" stroke="#8a939a" strokeWidth="1.2" />
      <rect x="187" y="234" width="20" height="20" rx="2" fill="#9aa3a9" stroke="#7d878e" strokeWidth="1" />
      <rect x="196" y="238" width="2" height="12" rx="1" fill="#2b3035" />
      <rect x="215" y="234" width="20" height="20" rx="2" fill="#9aa3a9" stroke="#7d878e" strokeWidth="1" />
      <rect x="224" y="238" width="2" height="12" rx="1" fill="#2b3035" />
      <rect x="199" y="261" width="24" height="7" rx="1.5" fill="#8a939a" stroke="#7d878e" strokeWidth=".8" />
      {/* Rodapé escuro, na largura do corpo */}
      <rect x="39" y="288" width="242" height="8" fill="#14181b" />
      </g>
    </svg>
  );
}
