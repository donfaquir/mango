import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Wan27FormValues } from "./types";

interface Wan27ParamsProps {
  onSubmit: (values: Wan27FormValues) => Promise<void> | void;
  submitting: boolean;
}

export function Wan27Params({ onSubmit, submitting }: Wan27ParamsProps) {
  const [prompt, setPrompt] = useState("");
  const [negativePrompt, setNegativePrompt] = useState("");
  const [n, setN] = useState(1);
  const [size, setSize] = useState<"1024*1024" | "2K">("2K");
  const [enableSequential, setEnableSequential] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!prompt.trim()) {
      setError("请输入 prompt");
      return;
    }
    setError(null);
    await onSubmit({
      prompt: prompt.trim(),
      negative_prompt: negativePrompt.trim() || undefined,
      n,
      size,
      enable_sequential: enableSequential,
    });
    // Reset form on success
    setPrompt("");
    setNegativePrompt("");
    setN(1);
    setSize("2K");
    setEnableSequential(false);
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
          placeholder="描述你想生成的图片内容..."
          rows={4}
        />
        {error && <p className="text-xs text-destructive">{error}</p>}
      </div>

      <details className="rounded-md border bg-muted/30 px-3 py-2 text-sm">
        <summary className="cursor-pointer select-none font-medium">
          高级参数
        </summary>
        <div className="mt-3 space-y-3">
          <div className="space-y-1.5">
            <label className="text-sm font-medium">负面 prompt（可选）</label>
            <Textarea
              value={negativePrompt}
              onChange={(e) => setNegativePrompt(e.target.value)}
              rows={2}
              placeholder="不希望出现的内容..."
            />
          </div>
          <div className="space-y-1.5">
            <label className="text-sm font-medium">生成张数 (n)</label>
            <Input
              type="number"
              min={1}
              max={4}
              value={n}
              onChange={(e) => setN(Number(e.target.value))}
            />
          </div>
          <div className="space-y-1.5">
            <label className="text-sm font-medium">分辨率</label>
            <Select value={size} onValueChange={(v) => setSize(v as typeof size)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="1024*1024">1024*1024</SelectItem>
                <SelectItem value="2K">2K</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="enable_sequential"
              checked={enableSequential}
              onChange={(e) => setEnableSequential(e.target.checked)}
              className="h-4 w-4 rounded border"
            />
            <label htmlFor="enable_sequential" className="text-sm font-medium">
              组图保持一致 (enable_sequential)
            </label>
          </div>
        </div>
      </details>

      <Button type="submit" disabled={submitting} className="w-full">
        {submitting ? "提交中..." : "提交生成"}
      </Button>
    </form>
  );
}
