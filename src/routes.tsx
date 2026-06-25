import { createBrowserRouter } from "react-router-dom";
import { AppLayout } from "./components/AppLayout";
import PrinterPage from "./pages/printer/PrinterPage";
import Settings from "./pages/settings/Settings";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      {
        index: true,
        element: <PrinterPage />,
      },
      {
        path: "settings",
        element: <Settings />,
      },
    ],
  },
]);
