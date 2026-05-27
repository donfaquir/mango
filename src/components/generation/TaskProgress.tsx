import { Progress } from "@/components/ui/progress";

interface TaskProgressProps {
  value: number;
}

export function TaskProgress({ value }: TaskProgressProps) {
  return (
    <div className="flex items-center gap-2">
      <Progress value={value} className="h-1.5 flex-1" />
      <span className="text-xs text-muted-foreground">{value}%</span>
    </div>
  );
}
