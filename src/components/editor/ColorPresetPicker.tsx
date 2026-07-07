import { useQuery } from "@tanstack/react-query";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { commands, type ColorEffectPresetInfo } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

const PRESET_COLORS: Record<string, string> = {
  warm: "#FF8C42",
  cool: "#4287f5",
  vintage: "#C4A35A",
  bw: "#808080",
  high_contrast: "#1a1a1a",
  low_saturation: "#A8A8A8",
  night: "#1a2744",
  sunset: "#E8713A",
  dreamy: "#D4A8E8",
  horror: "#3D1F1F",
};

interface ColorPresetPickerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSelect: (preset: ColorEffectPresetInfo) => void;
}

export function ColorPresetPicker({ open, onOpenChange, onSelect }: ColorPresetPickerProps) {
  const { data: presets = [] } = useQuery({
    queryKey: ["color-effect-presets"],
    queryFn: () => unwrap(commands.listColorEffectPresets()),
    staleTime: Infinity,
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xs">
        <DialogHeader>
          <DialogTitle>选择调色预设</DialogTitle>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-2 max-h-64 overflow-y-auto">
          {presets.map((p) => (
            <button
              key={p.id}
              className="flex items-center gap-2 p-2 rounded hover:bg-accent text-sm"
              onClick={() => {
                onSelect(p);
                onOpenChange(false);
              }}
            >
              <span
                className="size-6 rounded-full shrink-0 border border-border"
                style={{ backgroundColor: PRESET_COLORS[p.id] ?? "#888" }}
              />
              <span>{p.label}</span>
            </button>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
