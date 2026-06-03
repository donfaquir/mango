import { useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";
import { shotKeys } from "./useShots";
import { checkpointKeys } from "./useCheckpoint";

const AUTO_SAVE_INTERVAL_MS = 30 * 60 * 1000;

async function computeHash(data: string): Promise<string> {
  const encoded = new TextEncoder().encode(data);
  const buffer = await crypto.subtle.digest("SHA-256", encoded);
  return Array.from(new Uint8Array(buffer))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export function useCheckpointAutoSave(episodeId: string | undefined) {
  const qc = useQueryClient();
  const lastHashRef = useRef<string>("");

  useEffect(() => {
    if (!episodeId) return;

    const tick = async () => {
      try {
        const shotsData = qc.getQueryData(shotKeys.all(episodeId));
        const episodeData = qc.getQueryData<{ script_text?: string }>([
          "episode",
          episodeId,
        ]);
        const canvasData = qc.getQueryData<{
          nodes_json?: string;
          edges_json?: string;
        }>(["canvas-layout", episodeId]);

        const snapshot = JSON.stringify({
          script: episodeData?.script_text ?? "",
          shots: shotsData ?? [],
          canvas: canvasData?.nodes_json ?? "",
        });

        const hash = await computeHash(snapshot);
        if (hash === lastHashRef.current) return;

        await unwrap(
          commands.createEpisodeCheckpoint({
            episode_id: episodeId,
            label: null,
            trigger_type: "auto",
          }),
        );

        lastHashRef.current = hash;
        qc.invalidateQueries({ queryKey: checkpointKeys.all(episodeId) });
      } catch {
        // Auto-save failures are non-critical
      }
    };

    const id = setInterval(tick, AUTO_SAVE_INTERVAL_MS);
    return () => clearInterval(id);
  }, [episodeId, qc]);
}
