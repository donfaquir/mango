import { Outlet } from "react-router-dom";
import { WorkspaceNav } from "@/components/layout/WorkspaceNav";

export default function ProjectWorkspacePage() {
  // No per-project asset-protocol scope registration here anymore: the
  // workspace-wide scope is granted once at app startup (lib.rs setup),
  // which already covers every project under `<workspace>/projects/`.
  return (
    <div className="flex h-full flex-col gap-4">
      <WorkspaceNav />
      <div className="flex-1 overflow-auto">
        <Outlet />
      </div>
    </div>
  );
}
