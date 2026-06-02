import { Navigate, useParams } from "react-router-dom";
import { ReactFlowProvider } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { CanvasContainer } from "@/components/canvas/CanvasContainer";

export default function CanvasPage() {
  const { episodeId } = useParams<{ episodeId: string }>();
  if (!episodeId) return <Navigate to=".." replace />;

  return (
    <div className="h-full w-full">
      <ReactFlowProvider>
        <CanvasContainer episodeId={episodeId} />
      </ReactFlowProvider>
    </div>
  );
}
