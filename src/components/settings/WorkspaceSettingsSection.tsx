import { FolderOpen, HardDrive } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useWorkspaceStatus } from "@/hooks/useWorkspace";
import { WorkspacePicker } from "@/components/workspace/WorkspacePicker";

/**
 * Settings panel section that surfaces the current workspace and lets the
 * user switch to a different one. Switching goes through the same
 * pick → probe → confirm → relaunch flow as onboarding (reused via
 * `WorkspacePicker`); the only delta is a banner reminding the user that
 * the app will restart.
 */
export function WorkspaceSettingsSection() {
  const { data: status } = useWorkspaceStatus();
  const current = status?.workspace_root ?? "（未挂载）";

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <HardDrive className="size-4" />
          工作区
        </CardTitle>
        <CardDescription>
          当前所有项目数据、素材和缩略图都保存在此目录下。API 密钥仍在系统钥匙串。
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="rounded-md border bg-muted/40 px-3 py-2 font-mono text-xs break-all">
          {current}
        </div>
        <WorkspacePicker
          mode="switch"
          switchWarning="切换后应用会退出，需要手动重新启动以加载新工作区。当前工作区的数据保留在原目录，不会被删除。"
          trigger={({ onClick, busy }) => (
            <Button variant="outline" onClick={onClick} disabled={busy}>
              <FolderOpen className="mr-2 size-4" />
              {busy ? "处理中..." : "切换工作区..."}
            </Button>
          )}
        />
      </CardContent>
    </Card>
  );
}
