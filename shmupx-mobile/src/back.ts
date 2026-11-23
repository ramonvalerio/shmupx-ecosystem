import { useEffect, useRef } from "react";

/**
 * Botão voltar do Android: cada camada aberta (sala, ajustes, folha de Play) põe uma entrada no
 * histórico da página. O voltar do sistema volta no histórico do WebView, então fecha a camada de
 * cima em vez de fechar o app. Fechar pela própria tela tira a entrada sem disparar o fechamento.
 */
type Layer = { close: () => void };
const layers: Layer[] = [];
let ignoredPops = 0;

if (typeof window !== "undefined") {
  window.addEventListener("popstate", () => {
    if (ignoredPops > 0) { ignoredPops--; return; }
    layers.pop()?.close();
  });
}

export function useBackLayer(open: boolean, onBack: () => void) {
  const callback = useRef(onBack);
  callback.current = onBack;
  useEffect(() => {
    if (!open) return;
    const layer: Layer = { close: () => callback.current() };
    layers.push(layer);
    window.history.pushState({ liteLayer: layers.length }, "");
    return () => {
      const index = layers.indexOf(layer);
      if (index < 0) return; // já saiu pelo voltar do sistema
      layers.splice(index, 1);
      ignoredPops++;
      window.history.back();
    };
  }, [open]);
}
