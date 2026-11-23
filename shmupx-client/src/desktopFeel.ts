/**
 * O Client é um app desktop, não uma página web: sem menu do botão direito do navegador, sem
 * arrastar imagens e sem os atalhos de navegador (imprimir, procurar, salvar, voltar, zoom…).
 * A seleção de texto fica desligada no CSS (index.css), menos nos campos de digitação.
 * Na versão em desenvolvimento, recarregar (F5/Ctrl+R) e as ferramentas (F12/Ctrl+Shift+I) seguem.
 */
export function installDesktopFeel() {
  window.addEventListener("contextmenu", (event) => event.preventDefault(), { capture: true });
  window.addEventListener("dragstart", (event) => {
    if (event.target instanceof HTMLImageElement || event.target instanceof HTMLAnchorElement) event.preventDefault();
  }, { capture: true });
  // Botões "voltar/avançar" do mouse.
  window.addEventListener("mouseup", (event) => { if (event.button === 3 || event.button === 4) event.preventDefault(); }, { capture: true });
  // Zoom do navegador com Ctrl + roda do mouse.
  window.addEventListener("wheel", (event) => { if (event.ctrlKey) event.preventDefault(); }, { capture: true, passive: false });
  window.addEventListener("keydown", (event) => {
    const key = event.key.toLowerCase();
    const ctrl = event.ctrlKey || event.metaKey;
    const devTool = key === "f5" || key === "f12" || (ctrl && key === "r") || (ctrl && event.shiftKey && (key === "i" || key === "j"));
    if (devTool && import.meta.env.DEV) return;
    const blocked = devTool
      || ["f3", "f7", "browserback", "browserforward", "browserrefresh", "browsersearch"].includes(key)
      || (event.altKey && (key === "arrowleft" || key === "arrowright" || key === "home"))
      || (ctrl && ["p", "f", "g", "s", "u", "o", "j", "h", "l", "+", "-", "=", "0"].includes(key))
      || (ctrl && event.shiftKey && (key === "c" || key === "delete"));
    if (blocked) event.preventDefault();
  }, { capture: true });
}
