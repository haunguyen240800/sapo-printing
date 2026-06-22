import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router-dom";
import { AppProvider } from "@sapo/ui-components";
import translations from "@sapo/ui-components/locales/vi.json";

import { router } from "./routes";

import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <AppProvider i18n={translations}>
      <RouterProvider router={router} />
    </AppProvider>
  </React.StrictMode>
);
