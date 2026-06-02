import { Loader2 } from "lucide-react";
import { RouterProvider } from "react-router-dom";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useWorkspaceStatus } from "@/hooks/useWorkspace";
import { useTaskEventListener, useTaskStatusListener } from "@/hooks/useTasks";
import OnboardingPage from "@/pages/OnboardingPage";
import { router } from "./routes";

function App() {
  const { data: status, isLoading, error } = useWorkspaceStatus();

  // First paint: tiny spinner while the IPC round-trip resolves. Keeping this
  // outside both branches avoids flashing the onboarding screen at users who
  // already have a workspace mounted.
  if (isLoading) {
    return (
      <TooltipProvider delayDuration={300}>
        <div className="flex min-h-screen items-center justify-center">
          <Loader2 className="size-6 animate-spin text-muted-foreground" />
        </div>
        <Toaster richColors position="top-right" />
      </TooltipProvider>
    );
  }

  // Hard failure on the boot probe — surface it instead of silently
  // dropping into onboarding (which would mask "DB exists but is locked"
  // style problems).
  if (error) {
    return (
      <TooltipProvider delayDuration={300}>
        <div className="flex min-h-screen items-center justify-center px-6">
          <div className="max-w-md space-y-2 text-center">
            <p className="text-lg font-medium">无法启动 Mango</p>
            <p className="text-sm text-muted-foreground">
              检测工作区状态失败：{error.message}
            </p>
          </div>
        </div>
        <Toaster richColors position="top-right" />
      </TooltipProvider>
    );
  }

  const mounted = status?.workspace_root != null;

  return (
    <TooltipProvider delayDuration={300}>
      {mounted ? <MainApp /> : <OnboardingPage />}
      <Toaster richColors position="top-right" />
    </TooltipProvider>
  );
}

/**
 * Main router + task-engine event listeners. Lives in its own component so
 * the listener hooks are only mounted when a workspace is actually loaded
 * — onboarding mode runs against a `:memory:` DB with no real task engine,
 * and there's nothing to listen for.
 */
function MainApp() {
  useTaskStatusListener();
  useTaskEventListener();
  return <RouterProvider router={router} />;
}

export default App;
