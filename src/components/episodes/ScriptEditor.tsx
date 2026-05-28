import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Textarea } from "@/components/ui/textarea";
import { useUpdateEpisode } from "@/hooks/useEpisodes";

interface Props {
  projectId: string;
  episodeId: string;
  initialText: string;
}

export function ScriptEditor({ projectId, episodeId, initialText }: Props) {
  const [text, setText] = useState(initialText);
  const mutation = useUpdateEpisode(projectId);

  useEffect(() => {
    setText(initialText);
  }, [initialText]);

  const handleBlur = async () => {
    if (text === initialText) return;
    try {
      await mutation.mutateAsync({
        id: episodeId,
        input: { script_text: text },
      });
    } catch (err) {
      toast.error(
        `保存失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-muted-foreground">剧本正文</h2>
        {mutation.isPending && (
          <span className="text-xs text-muted-foreground">保存中…</span>
        )}
      </div>
      <Textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={handleBlur}
        rows={10}
        placeholder="在此撰写本集剧本…"
      />
    </div>
  );
}
