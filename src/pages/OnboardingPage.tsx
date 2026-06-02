import { FolderOpen, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { WorkspacePicker } from "@/components/workspace/WorkspacePicker";

/**
 * Full-screen first-run experience. Shown by `App.tsx` whenever
 * `useWorkspaceStatus()` reports no workspace mounted; once the user picks
 * one and confirms, `setWorkspaceAndRelaunch` restarts the process and the
 * next boot lands on the main app instead.
 */
export default function OnboardingPage() {
  return (
    <div className="flex min-h-screen items-center justify-center bg-muted/30 px-4">
      <Card className="w-full max-w-lg">
        <CardHeader className="space-y-2 text-center">
          <div className="mx-auto flex size-12 items-center justify-center rounded-full bg-primary/10 text-primary">
            <Sparkles className="size-6" />
          </div>
          <CardTitle className="text-2xl">欢迎使用 Mango</CardTitle>
          <p className="text-sm text-muted-foreground">
            选择一个目录作为你的<strong>工作区</strong>。所有项目数据、素材、缩略图
            都会保存在这里，方便日后整体备份或迁移到其他机器。
          </p>
        </CardHeader>
        <CardContent className="space-y-4">
          <ul className="space-y-2 text-sm text-muted-foreground">
            <li>· 可以选一个空目录，或选一个已有的 mango 工作区</li>
            <li>· 之后可在「设置」里随时切换</li>
            <li>· API 密钥仍存放在系统钥匙串，不写入工作区</li>
          </ul>
          <WorkspacePicker
            mode="mount"
            trigger={({ onClick, busy }) => (
              <Button
                size="lg"
                className="w-full"
                onClick={onClick}
                disabled={busy}
              >
                <FolderOpen className="mr-2 size-5" />
                {busy ? "处理中..." : "选择工作区目录"}
              </Button>
            )}
          />
        </CardContent>
      </Card>
    </div>
  );
}
