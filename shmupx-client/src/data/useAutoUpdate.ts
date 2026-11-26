import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/** Nova checagem enquanto o Client fica aberto (a liberação pode chegar com o app rodando). */
const CHECAR_A_CADA_MS = 30 * 60_000;

export type AutoUpdateState =
  | { status: "idle" }
  | { status: "downloading"; version: string; progress: number | null }
  | { status: "ready"; version: string; notes?: string }
  | { status: "installing"; version: string }
  | { status: "error"; message: string };

/**
 * Atualização automática: o backend só oferece versões liberadas no Lab com "Atualizar os
 * Clients automaticamente". O pacote é baixado em segundo plano; a instalação (que fecha o app no
 * Windows) espera o jogador escolher, e nunca acontece no meio de uma gravação.
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

  /** Fecha o app, instala em modo passivo e o instalador abre a versão nova. */
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
      setState({ status: "error", message: e instanceof Error ? e.message : String(e) });
    }
  }, [busy]);

  return { state, installNow };
}
