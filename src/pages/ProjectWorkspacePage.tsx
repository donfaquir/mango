import { useEffect } from "react";
import { Outlet, useParams } from "react-router-dom";
import { useRegisterProjectAssetScope } from "@/hooks/useAssets";
import { WorkspaceNav } from "@/components/layout/WorkspaceNav";

export default function ProjectWorkspacePage() {
  const { projectId } = useParams<{ projectId: string }>();
  const { mutate: registerScope } = useRegisterProjectAssetScope();

  // The asset-protocol scope is in-memory only; it must be re-registered every
  // time the app starts, otherwise `convertFileSrc` URLs for project files are
  // rejected and reference images fail to render after a restart.
  useEffect(() => {
    if (!projectId) return;
    registerScope(projectId);
  }, [projectId, registerScope]);

  return (
    <div className="flex h-full flex-col gap-4">
      <WorkspaceNav />
      <div className="flex-1 overflow-auto">
        <Outlet />
      </div>
    </div>
  );
}
