import { lazy } from "react";
import { createBrowserRouter } from "react-router-dom";
import { AppLayout } from "@/components/layout/AppLayout";

const ProjectListPage = lazy(() => import("@/pages/ProjectListPage"));
const ProjectWorkspacePage = lazy(
  () => import("@/pages/ProjectWorkspacePage"),
);
const SettingsPage = lazy(() => import("@/pages/SettingsPage"));

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      { index: true, element: <ProjectListPage /> },
      { path: "project/:projectId", element: <ProjectWorkspacePage /> },
      { path: "settings", element: <SettingsPage /> },
    ],
  },
]);
