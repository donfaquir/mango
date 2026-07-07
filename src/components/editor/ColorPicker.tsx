import { cn } from "@/lib/utils";
import { Label } from "@/components/ui/label";

interface ColorPickerProps {
  label: string;
  value: string;
  onChange: (color: string) => void;
  className?: string;
}

export function ColorPicker({ label, value, onChange, className }: ColorPickerProps) {
  return (
    <div className={cn("flex items-center gap-2", className)}>
      <Label className="text-xs shrink-0 w-10">{label}</Label>
      <input
        type="color"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-7 h-5 rounded border border-input cursor-pointer bg-transparent"
      />
      <span className="text-xs font-mono text-muted-foreground">{value}</span>
    </div>
  );
}
