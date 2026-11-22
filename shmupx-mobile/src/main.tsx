import React from "react";
import ReactDOM from "react-dom/client";
import { I18nProvider } from "@client/i18n";
import App from "./App";
import "./lite.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <I18nProvider><App /></I18nProvider>
  </React.StrictMode>,
);
