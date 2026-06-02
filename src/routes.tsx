import { lazy } from "react";
import { createBrowserRouter, Navigate } from "react-router-dom";
import { AppLayout } from "@/components/layout/AppLayout";
import { AccountsPanel } from "@/components/accounts/AccountsPanel";
import { GeneralSettingsPanel } from "@/components/settings/GeneralSettingsPanel";
// ProjectListPage is the index route — every cold start lands here, so lazy
// would only add a round-trip. The other pages stay lazy.
import ProjectListPage from "@/pages/ProjectListPage";
const ProjectWorkspacePage = lazy(
  () => import("@/pages/ProjectWorkspacePage"),
);
const SubjectLibraryPage = lazy(() => import("@/pages/SubjectLibraryPage"));
const SubjectDetailPage = lazy(() => import("@/pages/SubjectDetailPage"));
const EpisodeListPage = lazy(() => import("@/pages/EpisodeListPage"));
const EpisodeDetailPage = lazy(() => import("@/pages/EpisodeDetailPage"));
const CanvasPage = lazy(() => import("@/pages/CanvasPage"));
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
            path: "episodes",
            children: [
              { index: true, element: <EpisodeListPage /> },
              { path: ":episodeId", element: <EpisodeDetailPage /> },
              { path: ":episodeId/canvas", element: <CanvasPage /> },
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
