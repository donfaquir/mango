import { RouterProvider } from "react-router-dom";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useTaskEventListener, useTaskStatusListener } from "@/hooks/useTasks";
import { router } from "./routes";

function App() {
  useTaskStatusListener();
  useTaskEventListener();
  return (
    <TooltipProvider delayDuration={300}>
      <RouterProvider router={router} />
      <Toaster richColors position="top-right" />
    </TooltipProvider>
  );
}

export default App;
