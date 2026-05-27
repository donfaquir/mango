import { RouterProvider } from "react-router-dom";
import { Toaster } from "@/components/ui/sonner";
import { useTaskEventListener, useTaskStatusListener } from "@/hooks/useTasks";
import { router } from "./routes";

function App() {
  useTaskStatusListener();
  useTaskEventListener();
  return (
    <>
      <RouterProvider router={router} />
      <Toaster richColors position="top-right" />
    </>
  );
}

export default App;
