import React from "react";
import ReactDOM from "react-dom/client";
import { AppProvider } from "@sapo/ui-components";
import translations from "@sapo/ui-components/locales/vi.json";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <AppProvider i18n={translations}>
      <App />
    </AppProvider>
  </React.StrictMode>,
);
