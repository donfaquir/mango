import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { SubjectPickerDialog } from "../SubjectPickerDialog";
import { SubjectThumbnail } from "@/components/subjects/SubjectThumbnail";
import { useCharacterList } from "@/hooks/useCharacters";
import type { HappyhorseFormValues } from "./types";

interface HappyhorseParamsProps {
  projectId: string;
  onSubmit: (values: HappyhorseFormValues) => Promise<void> | void;
  submitting: boolean;
}

export function HappyhorseParams({
  projectId,
  onSubmit,
  submitting,
}: HappyhorseParamsProps) {
  const [prompt, setPrompt] = useState("");
  const [subjectIds, setSubjectIds] = useState<string[]>([]);
  const [resolution, setResolution] = useState<"480P" | "720P" | "1080P">("720P");
  const [ratio, setRatio] = useState<"16:9" | "9:16" | "1:1">("16:9");
  const [duration, setDuration] = useState<3 | 5 | 10>(5);
  const [error, setError] = useState<string | null>(null);

  const characters = useCharacterList(projectId);
  const selectedCharacters = (characters.data ?? []).filter((c) =>
    subjectIds.includes(c.id),
  );

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!prompt.trim()) {
      setError("请输入 prompt");
      return;
    }
    if (subjectIds.length === 0) {
      setError("至少选择一个主体");
      return;
    }
    // Verify selected subjects have reference images
    const hasRefImage = selectedCharacters.some((c) => c.reference_image_path);
    if (!hasRefImage) {
      setError("选中的主体没有任何参考图，无法用于视频生成");
      return;
    }
    setError(null);
    await onSubmit({
      prompt: prompt.trim(),
      subject_ids: subjectIds,
      resolution,
      ratio,
      duration,
    });
    // Reset form on success
    setPrompt("");
    setSubjectIds([]);
    setResolution("720P");
    setRatio("16:9");
    setDuration(5);
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div className="space-y-1.5">
        <label className="text-sm font-medium">
          Prompt <span className="text-destructive">*</span>
        </label>
        <Textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder="例如：小明翻开书本，抬头微笑"
          rows={4}
        />
      </div>

      <div className="space-y-1.5">
        <label className="text-sm font-medium">
          主体（参考图来源）<span className="text-destructive">*</span>
        </label>
        <SubjectPickerDialog
          projectId={projectId}
          selected={subjectIds}
          onChange={setSubjectIds}
        />
        {selectedCharacters.length > 0 && (
          <div className="flex gap-2 overflow-x-auto py-2">
            {selectedCharacters.map((c) => (
              <div key={c.id} className="w-16 shrink-0 space-y-1">
                <div className="h-16 w-16 overflow-hidden rounded-md">
                  <SubjectThumbnail
                    projectId={projectId}
                    relativePath={c.reference_image_path}
                  />
                </div>
                <p className="truncate text-xs text-muted-foreground">
                  {c.name}
                </p>
              </div>
            ))}
          </div>
        )}
      </div>

      {error && <p className="text-xs text-destructive">{error}</p>}

      <div className="grid grid-cols-3 gap-3">
        <div className="space-y-1.5">
          <label className="text-sm font-medium">分辨率</label>
          <Select
            value={resolution}
            onValueChange={(v) => setResolution(v as typeof resolution)}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="480P">480P</SelectItem>
              <SelectItem value="720P">720P</SelectItem>
              <SelectItem value="1080P">1080P</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1.5">
          <label className="text-sm font-medium">画幅</label>
          <Select
            value={ratio}
            onValueChange={(v) => setRatio(v as typeof ratio)}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="16:9">16:9</SelectItem>
              <SelectItem value="9:16">9:16</SelectItem>
              <SelectItem value="1:1">1:1</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1.5">
          <label className="text-sm font-medium">时长（秒）</label>
          <Select
            value={String(duration)}
            onValueChange={(v) => setDuration(Number(v) as typeof duration)}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="3">3</SelectItem>
              <SelectItem value="5">5</SelectItem>
              <SelectItem value="10">10</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <Button type="submit" disabled={submitting} className="w-full">
        {submitting ? "提交中..." : "提交生成"}
      </Button>
    </form>
  );
}
