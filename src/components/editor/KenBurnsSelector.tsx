import {
  ZoomIn, ZoomOut,
  MoveLeft, MoveRight, MoveUp, MoveDown,
  Maximize, Focus, X,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { useKenBurnsPresets } from "@/hooks/useKeyframes";
import { useMultiTrackStore } from "@/stores/multiTrackStore";

const PRESET_ICONS: Record<string, React.ElementType> = {
  slow_zoom_in: ZoomIn,
  slow_zoom_out: ZoomOut,
  pan_left: MoveLeft,
  pan_right: MoveRight,
  pan_up: MoveUp,
  pan_down: MoveDown,
  zoom_pan_right: Maximize,
  dramatic_zoom: Focus,
};

interface KenBurnsSelectorProps {
  selectedClipIds: string[];
  currentPresetId: string | null;
}

export function KenBurnsSelector({ selectedClipIds, currentPresetId }: KenBurnsSelectorProps) {
  const { data: presets } = useKenBurnsPresets();
  const updateItem = useMultiTrackStore((s) => s.updateItem);
  const items = useMultiTrackStore((s) => s.items);

  if (!presets || selectedClipIds.length === 0) return null;

  const applyPreset = (presetId: string | null) => {
    for (const id of selectedClipIds) {
      const item = items[id];
      if (!item) continue;
      const params = safeParseParams(item.params_json);
      params.ken_burns_preset = presetId;
      updateItem(id, {
        position_ms: null,
        duration_ms: null,
        in_point_ms: null,
        out_point_ms: null,
        params_json: JSON.stringify(params),
      });
    }
  };

  return (
    <div className="flex items-center gap-1 px-3 py-1 border-t border-border bg-muted/10 text-xs">
      <span className="text-muted-foreground shrink-0 mr-1">Ken Burns</span>
      <div className="flex gap-1 flex-wrap">
        {presets.map((p) => {
          const Icon = PRESET_ICONS[p.id] ?? ZoomIn;
          const active = currentPresetId === p.id;
          return (
            <Button
              key={p.id}
              variant={active ? "default" : "outline"}
              size="sm"
              className={cn("h-6 text-xs gap-1", active && "ring-1 ring-primary")}
              title={p.description}
              onClick={() => applyPreset(p.id)}
            >
              <Icon className="size-3" />
              {p.description}
            </Button>
          );
        })}
        {currentPresetId && (
          <Button
            variant="ghost"
            size="sm"
            className="h-6 text-xs gap-1 text-muted-foreground"
            onClick={() => applyPreset(null)}
          >
            <X className="size-3" />
            清除
          </Button>
        )}
      </div>
    </div>
  );
}

function safeParseParams(json: string): Record<string, unknown> {
  try {
    const parsed = JSON.parse(json) as Record<string, unknown>;
    return typeof parsed === "object" && parsed !== null ? parsed : { type: "Clip" };
  } catch {
    return { type: "Clip" };
  }
}
