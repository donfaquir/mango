import { useState } from "react";
import { Terminal } from "lucide-react";
import { toast } from "sonner";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { commands } from "@/lib/bindings/commands";
import { IpcCallError, unwrap } from "@/lib/ipc";

function useCliStatus() {
  return useQuery({
    queryKey: ["cli", "status"],
    queryFn: () => unwrap(commands.checkCliInstalled()),
  });
}

export function CliToolsSection() {
  const { data: status, isLoading } = useCliStatus();
  const queryClient = useQueryClient();
  const [busy, setBusy] = useState(false);

  const handleInstall = async () => {
    setBusy(true);
    try {
      await unwrap(commands.installCli());
      await queryClient.invalidateQueries({ queryKey: ["cli", "status"] });
      toast.success("命令行工具已安装，可在终端使用 mango 命令");
    } catch (err) {
      if (err instanceof IpcCallError && err.code === "USER_CANCELLED") return;
      toast.error(
        `安装失败：${err instanceof Error ? err.message : String(err)}`,
      );
    } finally {
      setBusy(false);
    }
  };

  const handleUninstall = async () => {
    setBusy(true);
    try {
      await unwrap(commands.uninstallCli());
      await queryClient.invalidateQueries({ queryKey: ["cli", "status"] });
      toast.success("命令行工具已卸载");
    } catch (err) {
      if (err instanceof IpcCallError && err.code === "USER_CANCELLED") return;
      toast.error(
        `卸载失败：${err instanceof Error ? err.message : String(err)}`,
      );
    } finally {
      setBusy(false);
    }
  };

  const installed = status?.installed ?? false;
  const current = status?.points_to_current_app ?? false;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <Terminal className="size-4" />
          命令行工具
        </CardTitle>
        <CardDescription>
          在终端中使用 <code className="text-xs">mango</code>{" "}
          命令管理项目、角色和生成任务。安装后会在 /usr/local/bin 创建符号链接。
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-3">
        {!isLoading && (
          <>
            <div className="rounded-md border bg-muted/40 px-3 py-2 text-sm">
              {!installed && "未安装"}
              {installed && current && (
                <span className="text-green-600 dark:text-green-400">
                  已安装 — {status?.symlink_target}
                </span>
              )}
              {installed && !current && (
                <span className="text-yellow-600 dark:text-yellow-400">
                  已安装但指向其他位置 — {status?.symlink_target}
                </span>
              )}
            </div>
            {!installed && (
              <Button
                variant="outline"
                onClick={handleInstall}
                disabled={busy}
              >
                {busy ? "安装中..." : "安装命令行工具"}
              </Button>
            )}
            {installed && !current && (
              <Button
                variant="outline"
                onClick={handleInstall}
                disabled={busy}
              >
                {busy ? "更新中..." : "更新为当前应用"}
              </Button>
            )}
            {installed && current && (
              <Button
                variant="outline"
                onClick={handleUninstall}
                disabled={busy}
              >
                {busy ? "卸载中..." : "卸载命令行工具"}
              </Button>
            )}
          </>
        )}
      </CardContent>
    </Card>
  );
}
