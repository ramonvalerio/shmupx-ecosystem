import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/** Nova checagem enquanto o Client fica aberto (a liberação pode chegar com o app rodando). */
const CHECAR_A_CADA_MS = 30 * 60_000;
/** Aviso antes de instalar sozinho, com "Atualizar agora" e "Depois". */
const INSTALAR_EM_S = 10;

export type AutoUpdateState =
  | { status: "idle" }
  | { status: "downloading"; version: string; progress: number | null }
  | { status: "ready"; version: string; notes?: string }
  | { status: "installing"; version: string }
  | { status: "error"; message: string };

/**
 * Atualização automática: o backend só oferece versões liberadas no Lab com "Atualizar os
 * Clients automaticamente". O pacote é baixado em segundo plano e, terminado o download, o Client
 * se atualiza sozinho depois de um aviso de alguns segundos (o jogador pode adiar). Nunca com um
 * jogo, gravação ou live em andamento (`busy`): espera acabar. No Windows o instalador roda em modo
 * silencioso (tauri.conf.json › updater.windows.installMode "quiet"), sem a tela do Setup, e reabre
 * o Client no fim.
 *
 * No ShmupX Portable o aviso é o mesmo, mas o pacote é o próximo ShmupX-Portable-<versão>.exe
 * (`portatil_baixar`, SHA-256 conferido no Rust): ao atualizar, ele abre e o Client fecha.
 */
export function useAutoUpdate(busy: boolean) {
  const [state, setState] = useState<AutoUpdateState>({ status: "idle" });
  const pending = useRef<Update | null>(null);
  const running = useRef(false);
  const portable = useRef<Promise<boolean> | null>(null);
  const isPortable = () => (portable.current ??= invoke<boolean>("portatil_ativo").catch(() => false));
  /** Segundos até instalar sozinho (null: sem contagem). */
  const [countdown, setCountdown] = useState<number | null>(null);
  /** O jogador escolheu "Depois": fica só o botão "Reiniciar e atualizar". */
  const [postponed, setPostponed] = useState(false);

  const checkNow = useCallback(async () => {
    if (running.current || pending.current) return;
    running.current = true;
    try {
      const update = await check();
      if (!update) return;
      let total: number | null = null;
      let received = 0;
      setState({ status: "downloading", version: update.version, progress: null });
      if (await isPortable()) {
        const stop = await listen<{ recebido: number; total: number | null }>("portatil://progresso", (event) => {
          const { recebido, total: size } = event.payload;
          setState({ status: "downloading", version: update.version, progress: size ? recebido / size : null });
        });
        try {
          await invoke("portatil_baixar", { versao: update.version });
        } finally {
          stop();
        }
        pending.current = update;
        setState({ status: "ready", version: update.version, notes: update.body });
        return;
      }
      await update.download((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? null;
        if (event.event === "Progress") {
          received += event.data.chunkLength;
          setState({ status: "downloading", version: update.version, progress: total ? received / total : null });
        }
      });
      pending.current = update;
      setState({ status: "ready", version: update.version, notes: update.body });
    } catch (e) {
      // Sem rede ou backend fora do ar: tenta de novo na próxima checagem, sem incomodar.
      console.warn("atualização automática:", e);
      setState((current) => (current.status === "downloading" ? { status: "idle" } : current));
    } finally {
      running.current = false;
    }
  }, []);

  useEffect(() => {
    void checkNow();
    const timer = window.setInterval(() => void checkNow(), CHECAR_A_CADA_MS);
    return () => window.clearInterval(timer);
  }, [checkNow]);

  /** Fecha o app, instala em modo silencioso e o instalador abre a versão nova. */
  const installNow = useCallback(async () => {
    const update = pending.current;
    if (!update || busy) return;
    setState({ status: "installing", version: update.version });
    try {
      if (await isPortable()) {
        // Abre o ShmupX Portable novo (ele descompacta e abre a versão nova) e fecha este.
        await invoke("portatil_instalar", { versao: update.version });
        return;
      }
      await update.install();
      // No Windows o instalador fecha e reabre o app; no Linux o pacote é trocado com o app aberto
      // e é preciso reiniciar para entrar na versão nova.
      await relaunch();
    } catch (e) {
      // Ex.: o jogador recusou a permissão do Windows. O pacote continua baixado: o botão fica.
      console.warn("instalar atualização:", e);
      setPostponed(true);
      setState({ status: "ready", version: update.version, notes: update.body });
    }
  }, [busy]);

  const install = useRef(installNow);
  install.current = installNow;
  const waiting = state.status === "ready" && !busy && !postponed;
  useEffect(() => {
    if (!waiting) {
      setCountdown(null);
      return;
    }
    let left = INSTALAR_EM_S;
    setCountdown(left);
    const timer = window.setInterval(() => {
      left -= 1;
      if (left > 0) {
        setCountdown(left);
        return;
      }
      window.clearInterval(timer);
      setCountdown(null);
      void install.current();
    }, 1000);
    return () => window.clearInterval(timer);
  }, [waiting]);

  const postpone = useCallback(() => setPostponed(true), []);

  return { state, installNow, countdown, postpone };
}
