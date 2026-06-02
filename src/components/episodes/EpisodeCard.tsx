import { Link } from "react-router-dom";
import type { Episode } from "@/lib/bindings/commands";

interface Props {
  projectId: string;
  episode: Episode;
  shotCount?: number;
}

export function EpisodeCard({ projectId, episode, shotCount }: Props) {
  return (
    <Link
      to={`/project/${projectId}/episodes/${episode.id}`}
      className="flex flex-col gap-2 rounded-lg border p-4 text-left transition-colors hover:bg-accent"
    >
      <div className="flex items-baseline justify-between gap-2">
        <h3 className="truncate text-base font-medium">{episode.title}</h3>
        <span className="shrink-0 text-xs text-muted-foreground">
          #{episode.order_index + 1}
        </span>
      </div>
      <p className="line-clamp-3 text-sm text-muted-foreground">
        {episode.script_text || "（暂无剧本内容）"}
      </p>
      {shotCount !== undefined && (
        <div className="text-xs text-muted-foreground">{shotCount} 个分镜</div>
      )}
    </Link>
  );
}
