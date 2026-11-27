import { useCallback, useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { markScreenForRestore } from "./estadoLocal";

/** Aviso antes de recarregar sozinho, com "Recarregar agora" e "Depois". */
const RECARREGAR_EM_S = 10;

/**
 * Interface nova publicada sem instalador (src-tauri/src/interface.rs): o Client baixa em segundo
 * plano e avisa (`interface://pronta`). Aqui ela entra com um recarregar rápido depois de alguns
 * segundos de aviso, nunca com jogo, gravação ou live em andamento (`busy`). "Depois" deixa para o
 * próximo início do Client, que já abre com ela.
 *
 * Ao montar, confirma que esta interface abriu: uma interface baixada que não confirma volta para a
 * embutida sozinha.
 */
export function useInterfaceUpdate(busy: boolean) {
  const [ready, setReady] = useState<string | null>(null);
  const [countdown, setCountdown] = useState<number | null>(null);
  const [postponed, setPostponed] = useState(false);
  const [applying, setApplying] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    void invoke("interface_confirmar").catch(() => {});
    const stop = listen<{ versao: string }>("interface://pronta", (event) => setReady(event.payload.versao));
    return () => { void stop.then((unlisten) => unlisten()); };
  }, []);

  const applyNow = useCallback(async () => {
    if (!ready || busy) return;
    setApplying(true);
    try {
      await invoke("interface_aplicar");
      // Volta à mesma tela com a interface nova.
      markScreenForRestore();
      window.location.reload();
    } catch (e) {
      // ex.: a interface baixada deixou de valer; fica a atual, sem incomodar
      console.warn("interface nova:", e);
      setReady(null);
      setApplying(false);
    }
  }, [ready, busy]);

  const apply = useRef(applyNow);
  apply.current = applyNow;
  const waiting = ready !== null && !busy && !postponed && !applying;
  useEffect(() => {
    if (!waiting) {
      setCountdown(null);
      return;
    }
    let left = RECARREGAR_EM_S;
    setCountdown(left);
    const timer = window.setInterval(() => {
      left -= 1;
      if (left > 0) {
        setCountdown(left);
        return;
      }
      window.clearInterval(timer);
      setCountdown(null);
      void apply.current();
    }, 1000);
    return () => window.clearInterval(timer);
  }, [waiting]);

  const postpone = useCallback(() => setPostponed(true), []);

  return { ready: ready !== null && !postponed, countdown, applying, applyNow, postpone };
}
