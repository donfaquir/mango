import { useCallback, useEffect, useRef } from "react";
import { Skeleton } from "@/components/ui/skeleton";
import { ErrorAlert } from "@/components/common/ErrorAlert";
import { useCanvasStore } from "@/stores/canvasStore";
import { useCanvasLayoutQuery } from "@/hooks/useCanvasLayout";
import { events } from "@/lib/bindings/commands";
import { CanvasInner } from "./CanvasInner";
import { deserializeLayout } from "./persistence/serialization";

interface Props {
  episodeId: string;
}

export function CanvasContainer({ episodeId }: Props) {
  const layoutQuery = useCanvasLayoutQuery(episodeId);
  const init = useCanvasStore((s) => s.init);
  const initedEpisodeRef = useRef<string | null>(null);

  // Init the store exactly once per episodeId. Re-firing on every
  // layoutQuery.data reference change (e.g. after autoSave's setQueryData
  // success) would clear the zundo undo stack via init()'s t.clear() AND
  // overwrite the live store with stale snapshots, defeating autoSave's
  // hash skip and re-persisting old data.
  useEffect(() => {
    if (initedEpisodeRef.current === episodeId) return;
    if (layoutQuery.data === undefined) return;
    initedEpisodeRef.current = episodeId;
    const raw = deserializeLayout(layoutQuery.data ?? null);
    init(raw);
  }, [layoutQuery.data, init, episodeId]);

  // After a checkpoint restore, the backend overwrites canvas_layout in DB.
  // Reset the guard so the next layoutQuery.data change re-inits the store.
  const handleRestored = useCallback(
    (eid: string) => {
      if (eid === episodeId) {
        initedEpisodeRef.current = null;
      }
    },
    [episodeId],
  );

  useEffect(() => {
    const unlisten = events.episodeDataRestored.listen((event) => {
      handleRestored(event.payload.episode_id);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [handleRestored]);

  // NOTE: do not reset() on unmount. React 19 runs parent cleanups before
  // child cleanups, so a reset() here would clear the store while the
  // CanvasInner's useAutoSave subscriber is still alive, triggering a final
  // save() that flushes "[]" to disk and wipes the episode. The next mount's
  // init() above will overwrite any stale store contents.

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
