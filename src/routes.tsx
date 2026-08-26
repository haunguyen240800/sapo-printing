import { lazy } from "react";
import { createBrowserRouter, Navigate } from "react-router-dom";

import { AppLayout } from "./components/AppLayout";
import { withSuspense } from "./utils/withSuspense";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      {
        index: true,
        element: <Navigate to="printer" replace />,
      },
      {
        path: "printer",
        element: withSuspense(lazy(() => import("./pages/printer/PrinterPage"))),
      },
      {
        path: "settings",
        element: withSuspense(lazy(() => import("./pages/settings/Settings"))),
      },
    ],
  },
]);
