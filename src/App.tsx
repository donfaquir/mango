import { RouterProvider } from "react-router-dom";
import { Toaster } from "@/components/ui/sonner";
import { useTaskStatusListener } from "@/hooks/useTasks";
import { router } from "./routes";

function App() {
  useTaskStatusListener();
  return (
    <>
      <RouterProvider router={router} />
      <Toaster richColors position="top-right" />
    </>
  );
}

export default App;
