import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import WatchWindow, { watchParams } from "./components/WatchWindow";
import { I18nProvider } from "./i18n";
import { installDesktopFeel } from "./desktopFeel";
import "./index.css";
import "./room.css";
import "./ranking.css";
import "./collection.css";
import "./theme-light.css";

installDesktopFeel();

// A mesma página serve a janela da live de um jogador (`?watch=<uid>&room=<gameId>`).
const watch = watchParams();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <I18nProvider>{watch ? <WatchWindow {...watch} /> : <App />}</I18nProvider>
  </React.StrictMode>,
);
