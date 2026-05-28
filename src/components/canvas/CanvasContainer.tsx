import { useEffect } from "react";
import { Skeleton } from "@/components/ui/skeleton";
import { ErrorAlert } from "@/components/common/ErrorAlert";
import { useCanvasStore } from "@/stores/canvasStore";
import { useCanvasLayoutQuery } from "@/hooks/useCanvasLayout";
import { CanvasInner } from "./CanvasInner";
import { deserializeLayout } from "./persistence/serialization";

interface Props {
  episodeId: string;
}

export function CanvasContainer({ episodeId }: Props) {
  const layoutQuery = useCanvasLayoutQuery(episodeId);
  const init = useCanvasStore((s) => s.init);
  const reset = useCanvasStore((s) => s.reset);

  useEffect(() => {
    if (layoutQuery.data === undefined) return;
    const raw = deserializeLayout(layoutQuery.data ?? null);
    init(raw);
  }, [layoutQuery.data, init]);

  useEffect(() => {
    return () => {
      // Clear the global canvas store when the user navigates away so a later
      // mount for a different episode starts from a clean slate.
      reset();
    };
  }, [reset]);

  if (layoutQuery.isLoading) {
    return <Skeleton className="h-full w-full" />;
  }
  if (layoutQuery.isError) {
    return (
      <ErrorAlert
        error={layoutQuery.error}
        onRetry={() => layoutQuery.refetch()}
      />
    );
  }

  const initialViewport = deserializeLayout(layoutQuery.data ?? null).viewport;

  return (
    <CanvasInner episodeId={episodeId} initialViewport={initialViewport} />
  );
}
