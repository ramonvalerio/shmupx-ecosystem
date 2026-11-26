import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { useDialogFocus } from "./useDialogFocus";
import { CONTROLLER_ACTIONS, bindingIdentity, readMappings, removeMapping, saveMapping, validateMapping, type ControllerAction, type ControllerBinding } from "./controllerMappings";
import DialogClose from "./DialogClose";

const ACTION_LABELS: Record<ControllerAction,string> = {up:"Cima",down:"Baixo",left:"Esquerda",right:"Direita",confirm:"Confirmar",back:"Voltar / fechar"};
interface Device {id:string;index:number;}

export default function ConfigureControllerDialog({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const [devices,setDevices] = useState<Device[]>([]);
  const [deviceId,setDeviceId] = useState("");
  const [bindings,setBindings] = useState<Partial<Record<ControllerAction,ControllerBinding>>>({});
  const [capturing,setCapturing] = useState<ControllerAction|null>(null);
  const [message,setMessage] = useState("");
  const [dirty,setDirty] = useState(false);
  const dialogRef = useDialogFocus(true, () => { if (capturing) setCapturing(null); else onClose(); });
  function scan() {
    try {
      const pads = navigator.getGamepads ? [...navigator.getGamepads()].filter((pad):pad is Gamepad => pad !== null && pad.connected) : [];
      setDevices(pads.map((pad)=>({id:pad.id,index:pad.index})));
      setMessage(pads.length ? "Selecione o controle e configure os seis comandos." : "Nenhum controle detectado. Conecte-o e pressione um botão antes de consultar novamente.");
    } catch { setMessage("A leitura de controles não está disponível neste ambiente."); }
  }
  useEffect(scan, []);
  useEffect(()=>{
    if (!capturing && dialogRef.current && !dialogRef.current.contains(document.activeElement)) dialogRef.current.querySelector<HTMLButtonElement>("[data-dialog-close]")?.focus();
  },[capturing,dialogRef]);
  useEffect(() => {
    if (!capturing || !deviceId) return;
    let frame = 0;
    let armed = false;
    const startedAt = performance.now();
    function poll(now:number) {
      if (now - startedAt > 15_000) { setCapturing(null); setMessage("Tempo de configuração esgotado. Tente novamente."); return; }
      if (!document.hasFocus() || document.hidden) { armed = false; frame = requestAnimationFrame(poll); return; }
      let pad: Gamepad | null | undefined;
      try { pad = navigator.getGamepads ? [...navigator.getGamepads()].find((candidate)=>candidate?.connected && candidate.id === deviceId) : null; }
      catch { setCapturing(null); setMessage("A leitura de controles não está disponível neste ambiente."); return; }
      if (!pad) { setCapturing(null); setMessage("O controle foi desconectado. Atualize a lista e tente novamente."); return; }
      const button = pad.buttons.findIndex((item)=>item.pressed);
      const axis = pad.axes.findIndex((value)=>Math.abs(value)>0.65);
      if (!armed) { if(button<0 && axis<0) armed=true; frame=requestAnimationFrame(poll); return; }
      if(button<0 && axis<0) { frame=requestAnimationFrame(poll); return; }
      const binding:ControllerBinding = button>=0 ? {kind:"button",index:button} : {kind:"axis",index:axis,direction:pad.axes[axis]>0?1:-1};
      if (Object.entries(bindings).some(([action,current])=>action!==capturing && current && bindingIdentity(current)===bindingIdentity(binding))) setMessage("Esse comando físico já está atribuído a outra ação.");
      else { setBindings((current)=>({...current,[capturing!]:binding})); setDirty(true); setMessage("Comando capturado. Salve para aplicar ao Big Picture."); }
      setCapturing(null);
    }
    frame=requestAnimationFrame(poll);
    return ()=>cancelAnimationFrame(frame);
  },[capturing,deviceId,bindings]);
  const valid = validateMapping({deviceId,bindings});
  function chooseDevice(id:string) { setDeviceId(id); setBindings(readMappings().find((mapping)=>mapping.deviceId===id)?.bindings ?? {}); setCapturing(null); setDirty(false); setMessage(""); }
  function describe(binding:ControllerBinding|undefined) { return !binding ? t("Não configurado") : binding.kind==="button" ? t("Botão {index}",{index:binding.index+1}) : t("Eixo {index} ({direction})",{index:binding.index+1,direction:binding.direction===1?"+":"−"}); }
  return <div className="controller-dialog-backdrop">
    <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby="controller-dialog-title" tabIndex={-1} data-capturing-controller={capturing ? "true" : undefined} className="controller-dialog">
      <header><div><h2 id="controller-dialog-title">{t("Configurar controle")}</h2><p>{t("Mapeie os comandos de navegação do Big Picture neste dispositivo.")}</p></div><DialogClose data-dialog-close label={capturing ? t("Cancelar captura") : undefined} onClick={()=>{ if(capturing)setCapturing(null);else onClose(); }} /></header>
      <div className="controller-dialog-body">
        <label className="settings-field">{t("Dispositivo de entrada")}<select value={deviceId} disabled={capturing!==null} onChange={(event)=>chooseDevice(event.target.value)}><option value="">{t("Selecione um controle")}</option>{devices.map((device)=><option key={device.index} value={device.id}>{device.id}</option>)}</select></label>
        <button className="text-action" disabled={capturing!==null} onClick={scan}>{t("Atualizar controles")}</button>
        <p className="muted">{t("Solte todos os botões e centralize o analógico antes de configurar cada ação. Depois pressione o botão ou mova o eixo desejado.")}</p>
        <div className="controller-mapping-list">{CONTROLLER_ACTIONS.map((action)=><div className="controller-mapping-row" key={action}><span>{t(ACTION_LABELS[action])}</span><span>{describe(bindings[action])}</span><button className="secondary-action" disabled={!deviceId || capturing!==null} onClick={()=>{setCapturing(action);setMessage("");}}>{t("Configurar {action}",{action:t(ACTION_LABELS[action])})}</button></div>)}</div>
        {capturing && <p role="status" className="controller-capture-notice">{t("Aguardando comando para {action}…",{action:t(ACTION_LABELS[capturing])})}</p>}
        <p role="status" className="muted">{t(message)}</p>
        <p className="muted">{t("Este perfil controla apenas a interface. Não remapeia os comandos do jogo nem o botão de comunicação por voz.")}</p>
      </div>
      <footer><button className="secondary-action" disabled={!deviceId || capturing!==null} onClick={()=>{if(removeMapping(deviceId)){setBindings({});setDirty(false);setMessage("Perfil removido. O controle volta ao mapeamento padrão quando disponível.");}else setMessage("Não foi possível salvar o perfil neste dispositivo.");}}>{t("Remover perfil")}</button><button className="primary-action" disabled={!valid || !dirty || capturing!==null} onClick={()=>{if(valid&&saveMapping(valid)){setDirty(false);setMessage("Perfil salvo. Os comandos estão disponíveis no Big Picture.");}else setMessage("Não foi possível salvar o perfil neste dispositivo.");}}>{t("Salvar perfil do controle")}</button></footer>
    </div>
  </div>;
}
