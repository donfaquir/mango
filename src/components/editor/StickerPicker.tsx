import { useQuery } from "@tanstack/react-query";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { commands, type StickerPresetInfo } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

const CATEGORY_LABELS: Record<string, string> = {
  emotion: "表情",
  effect: "特效",
  frame: "边框",
};

interface StickerPickerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSelect: (sticker: StickerPresetInfo) => void;
}

export function StickerPicker({ open, onOpenChange, onSelect }: StickerPickerProps) {
  const { data: presets = [] } = useQuery({
    queryKey: ["sticker-presets"],
    queryFn: () => unwrap(commands.listStickerPresets()),
    staleTime: Infinity,
  });

  const grouped = presets.reduce<Record<string, StickerPresetInfo[]>>((acc, p) => {
    (acc[p.category] ??= []).push(p);
    return acc;
  }, {});

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>选择贴纸</DialogTitle>
        </DialogHeader>
        <div className="space-y-3 max-h-64 overflow-y-auto">
          {Object.entries(grouped).map(([cat, items]) => (
            <div key={cat}>
              <h4 className="text-xs font-medium text-muted-foreground mb-1">
                {CATEGORY_LABELS[cat] ?? cat}
              </h4>
              <div className="grid grid-cols-5 gap-1.5">
                {items.map((s) => (
                  <button
                    key={s.id}
                    className="flex flex-col items-center gap-0.5 p-1.5 rounded hover:bg-accent text-xs"
                    onClick={() => {
                      onSelect(s);
                      onOpenChange(false);
                    }}
                  >
                    <span className="text-lg">
                      {s.id === "heart" ? "❤️" : s.id === "sparkle_eyes" ? "🤩" : s.id === "question_mark" ? "❓" : s.id === "anger_vein" ? "💢" : s.id === "sweat_drop" ? "💧" : s.id === "ellipsis" ? "💬" : s.id === "shock_lines" ? "💥" : s.id === "speed_lines" ? "💨" : s.id === "light_burst" ? "✨" : "🖼️"}
                    </span>
                    <span className="truncate w-full text-center">{s.label}</span>
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
