import { lazy } from "react";
import { createBrowserRouter, Navigate } from "react-router-dom";
import { AppLayout } from "@/components/layout/AppLayout";
import { AccountsPanel } from "@/components/accounts/AccountsPanel";
import { GeneralSettingsPanel } from "@/components/settings/GeneralSettingsPanel";

const ProjectListPage = lazy(() => import("@/pages/ProjectListPage"));
const ProjectWorkspacePage = lazy(
  () => import("@/pages/ProjectWorkspacePage"),
);
const SubjectLibraryPage = lazy(() => import("@/pages/SubjectLibraryPage"));
const SubjectDetailPage = lazy(() => import("@/pages/SubjectDetailPage"));
const GenerationWorkspacePage = lazy(
  () => import("@/pages/GenerationWorkspacePage"),
);
const AssetLibraryPage = lazy(() => import("@/pages/AssetLibraryPage"));
const SettingsPage = lazy(() => import("@/pages/SettingsPage"));

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      { index: true, element: <ProjectListPage /> },
      {
        path: "project/:projectId",
        element: <ProjectWorkspacePage />,
        children: [
          { index: true, element: <Navigate to="subjects/character" replace /> },
          {
            path: "subjects",
            children: [
              { index: true, element: <Navigate to="character" replace /> },
              { path: ":kind", element: <SubjectLibraryPage /> },
              {
                path: ":kind/:subjectId",
                element: <SubjectDetailPage />,
              },
            ],
          },
          {
            path: "assets",
            element: <AssetLibraryPage />,
          },
          {
            path: "generation",
            element: <GenerationWorkspacePage />,
          },
        ],
      },
      {
        path: "settings",
        element: <SettingsPage />,
        children: [
          { index: true, element: <Navigate to="general" replace /> },
          { path: "general", element: <GeneralSettingsPanel /> },
          { path: "accounts", element: <AccountsPanel /> },
        ],
      },
    ],
  },
]);
