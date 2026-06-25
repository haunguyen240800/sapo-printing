import React from "react";
import ReactDOM from "react-dom/client";
import { AppProvider } from "@sapo/ui-components";
import translations from "@sapo/ui-components/locales/vi.json";
import { RouterProvider } from "react-router-dom";
import { router } from "./routes";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <AppProvider i18n={translations}>
        <RouterProvider router={router} />
    </AppProvider>
  </React.StrictMode>,
);
