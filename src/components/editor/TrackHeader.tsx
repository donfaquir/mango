import { Volume2, VolumeX, Lock, Unlock, Film, Music, Type, Layers, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { TimelineTrack } from "@/lib/bindings/commands";

interface TrackHeaderProps {
  track: TimelineTrack;
  onToggleMute: () => void;
  onToggleLock: () => void;
  onDelete: () => void;
}

const TRACK_ICONS: Record<string, React.ElementType> = {
  video: Film,
  audio: Music,
  text: Type,
  overlay: Layers,
};

export function TrackHeader({ track, onToggleMute, onToggleLock, onDelete }: TrackHeaderProps) {
  const Icon = TRACK_ICONS[track.track_type] ?? Layers;

  return (
    <div className="flex items-center gap-1 px-2 h-16 border-b border-border bg-muted/30">
      <Icon className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
      <span className="text-xs font-medium truncate flex-1 min-w-0">{track.label}</span>
      <Button
        variant="ghost"
        size="icon"
        className={cn("h-6 w-6", track.muted && "text-destructive")}
        onClick={onToggleMute}
      >
        {track.muted ? <VolumeX className="h-3 w-3" /> : <Volume2 className="h-3 w-3" />}
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className={cn("h-6 w-6", track.locked && "text-orange-500")}
        onClick={onToggleLock}
      >
        {track.locked ? <Lock className="h-3 w-3" /> : <Unlock className="h-3 w-3" />}
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="h-6 w-6 text-muted-foreground hover:text-destructive"
        onClick={onDelete}
      >
        <Trash2 className="h-3 w-3" />
      </Button>
    </div>
  );
}
