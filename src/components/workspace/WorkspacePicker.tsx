import { useState } from "react";
import { FolderOpen, AlertTriangle, FolderCheck, FolderPlus } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { commands, type WorkspaceProbe } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";
import {
  useMountWorkspace,
  useProbeWorkspace,
  useSetWorkspaceAndRelaunch,
} from "@/hooks/useWorkspace";

/**
 * Two distinct flows live here:
 *
 * - `mode="mount"` (onboarding) — backend hot-mounts the workspace into
 *   `MountedState`, returns the new status, and the React Query cache
 *   update flips App.tsx into the main router. No restart, no toast prompt.
 *
 * - `mode="switch"` (settings) — backend writes the pointer config and
 *   exits; user relaunches. Hot-swap isn't feasible because the running
 *   app's React Query cache + router are bound to the current workspace.
 */
type Mode = "mount" | "switch";

interface WorkspacePickerProps {
  mode: Mode;
  /** Render prop for the trigger button — controls the visual placement. */
  trigger: (props: { onClick: () => void; busy: boolean }) => React.ReactNode;
  /**
   * Optional warning shown above the confirmation copy. Used in switch
   * mode to remind the user the app will exit and they'll need to
   * relaunch.
   */
  switchWarning?: string;
}

type Pending = { path: string; probe: WorkspaceProbe } | null;

export function WorkspacePicker({ mode, trigger, switchWarning }: WorkspacePickerProps) {
  const [pending, setPending] = useState<Pending>(null);
  const probe = useProbeWorkspace();
  const mount = useMountWorkspace();
  const relaunch = useSetWorkspaceAndRelaunch();
  const committing = mode === "mount" ? mount.isPending : relaunch.isPending;
  const busy = probe.isPending || committing;

  const handlePick = async () => {
    try {
      const picked = await unwrap(commands.pickProjectDirectory());
      if (!picked) return; // user cancelled
      const result = await probe.mutateAsync(picked);
      if (result.kind === "invalid") {
        toast.error(`目录不可用：${result.reason}`);
        return;
      }
      setPending({ path: picked, probe: result });
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err));
    }
  };

  const handleConfirm = async () => {
    if (!pending) return;
    try {
      if (mode === "mount") {
        // Hot-mount: backend installs MountedState into the OnceLock and
        // returns the new status; the hook updates React Query cache so
        // App.tsx re-renders into the main router automatically.
        await mount.mutateAsync(pending.path);
        toast.success("工作区已就绪");
      } else {
        // Switch: backend exits ~200ms after responding so the user
        // relaunches into the new workspace.
        await relaunch.mutateAsync(pending.path);
        toast.success("已保存工作区配置，应用将退出。请重新启动以加载新工作区。", {
          duration: 5000,
        });
      }
      setPending(null);
    } catch (err) {
      toast.error(`挂载失败：${err instanceof Error ? err.message : String(err)}`);
      setPending(null);
    }
  };

  return (
    <>
      {trigger({ onClick: handlePick, busy })}
      <ConfirmDialog
        pending={pending}
        mode={mode}
        switchWarning={switchWarning}
        committing={committing}
        onCancel={() => setPending(null)}
        onConfirm={handleConfirm}
      />
    </>
  );
}

interface ConfirmDialogProps {
  pending: Pending;
  mode: Mode;
  switchWarning?: string;
  committing: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

function ConfirmDialog({
  pending,
  mode,
  switchWarning,
  committing,
  onCancel,
  onConfirm,
}: ConfirmDialogProps) {
  if (!pending) return null;
  const variant = describe(pending.probe, mode);

  return (
    <Dialog open onOpenChange={(open) => !open && !committing && onCancel()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <variant.Icon className="size-5" />
            {variant.title}
          </DialogTitle>
          <DialogDescription className="break-all">{pending.path}</DialogDescription>
        </DialogHeader>
        <div className="space-y-2 text-sm">
          <p>{variant.body}</p>
          {switchWarning && (
            <p className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-900 dark:border-amber-900/40 dark:bg-amber-900/20 dark:text-amber-200">
              {switchWarning}
            </p>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel} disabled={committing}>
            取消
          </Button>
          <Button onClick={onConfirm} disabled={committing}>
            {committing ? variant.busyLabel : variant.confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function describe(probe: WorkspaceProbe, mode: Mode): {
  Icon: typeof FolderOpen;
  title: string;
  body: string;
  confirmLabel: string;
  busyLabel: string;
} {
  const busyLabel = mode === "mount" ? "正在挂载..." : "正在退出...";
  switch (probe.kind) {
    case "empty":
      return {
        Icon: FolderPlus,
        title: "初始化新工作区",
        body:
          mode === "mount"
            ? "这个目录是空的，将在其中创建 mango.db 与 projects/ 子目录，然后立即进入主界面。"
            : "这个目录是空的，将在其中创建 mango.db 与 projects/ 子目录。保存后应用会退出，请手动重新启动以加载新工作区。",
        confirmLabel: mode === "mount" ? "在此初始化" : "保存并退出",
        busyLabel,
      };
    case "existing_mango_data":
      return {
        Icon: FolderCheck,
        title: "挂载已有工作区",
        body:
          mode === "mount"
            ? `检测到 mango.db，里面包含 ${probe.project_count} 个项目。挂载后立即进入主界面。`
            : `检测到 mango.db，里面包含 ${probe.project_count} 个项目。保存后应用会退出，请手动重新启动以加载这份数据。`,
        confirmLabel: mode === "mount" ? "挂载" : "保存并退出",
        busyLabel,
      };
    case "non_empty_foreign":
      return {
        Icon: AlertTriangle,
        title: "目录非空",
        body:
          mode === "mount"
            ? "这个目录已有其他文件但不是 mango 工作区。继续会在其中创建 mango.db；如有疑虑请先备份或换个空目录。"
            : "这个目录已有其他文件但不是 mango 工作区。继续会在其中创建 mango.db；保存后应用会退出，请手动重新启动。",
        confirmLabel: "仍在此初始化",
        busyLabel,
      };
    case "invalid":
      // Should have been intercepted by the caller; render a neutral fallback.
      return {
        Icon: AlertTriangle,
        title: "目录不可用",
        body: probe.reason,
        confirmLabel: "重新选择",
        busyLabel,
      };
  }
}
