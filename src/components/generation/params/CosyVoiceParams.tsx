import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { CosyVoiceFormValues } from "./types";

const VOICE_OPTIONS = [
  { id: "longanyang", label: "龙安洋", desc: "男 · 阳光大男孩" },
  { id: "longanhuan_v3", label: "龙安欢", desc: "女 · 欢脱元气" },
  { id: "longxiaochun_v3", label: "龙小淳", desc: "女 · 知性积极" },
  { id: "longshu_v3", label: "龙书", desc: "男 · 沉稳青年" },
  { id: "longfei_v3", label: "龙飞", desc: "男 · 热血磁性" },
  { id: "longwan_v3", label: "龙婉", desc: "女 · 细腻柔声" },
  { id: "longyue_v3", label: "龙悦", desc: "女 · 温暖磁性" },
  { id: "longmiao_v3", label: "龙妙", desc: "女 · 抑扬顿挫" },
  { id: "longsanshu_v3", label: "龙三叔", desc: "男 · 沉稳质感" },
  { id: "longcheng_v3", label: "龙橙", desc: "男 · 智慧青年" },
  { id: "longhuhu_v3", label: "龙呼呼", desc: "女童 · 天真烂漫" },
  { id: "longjielidou_v3", label: "龙杰力豆", desc: "男童 · 阳光顽皮" },
] as const;

interface CosyVoiceParamsProps {
  onSubmit: (values: CosyVoiceFormValues) => Promise<void> | void;
  submitting: boolean;
}

export function CosyVoiceParams({ onSubmit, submitting }: CosyVoiceParamsProps) {
  const [text, setText] = useState("");
  const [voiceId, setVoiceId] = useState("longanyang");
  const [rate, setRate] = useState(1.0);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!text.trim()) {
      setError("请输入要合成的文本");
      return;
    }
    if (text.length > 5000) {
      setError("文本长度不能超过 5000 字符");
      return;
    }
    setError(null);
    await onSubmit({
      text: text.trim(),
      voice_id: voiceId,
      rate,
    });
    setText("");
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div className="space-y-1.5">
        <label className="text-sm font-medium">
          合成文本 <span className="text-destructive">*</span>
        </label>
        <Textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="输入要合成语音的文本..."
          rows={4}
          maxLength={5000}
        />
        <div className="flex justify-between text-xs text-muted-foreground">
          {error ? (
            <p className="text-destructive">{error}</p>
          ) : (
            <span />
          )}
          <span>{text.length}/5000</span>
        </div>
      </div>

      <div className="space-y-1.5">
        <label className="text-sm font-medium">
          音色 <span className="text-destructive">*</span>
        </label>
        <Select value={voiceId} onValueChange={setVoiceId}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {VOICE_OPTIONS.map((v) => (
              <SelectItem key={v.id} value={v.id}>
                {v.label}
                <span className="ml-2 text-muted-foreground text-xs">
                  {v.desc}
                </span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="space-y-1.5">
        <label className="text-sm font-medium">语速</label>
        <div className="flex items-center gap-2">
          <Input
            type="number"
            min={0.5}
            max={2.0}
            step={0.1}
            value={rate}
            onChange={(e) => setRate(Number(e.target.value))}
            className="w-24"
          />
          <span className="text-xs text-muted-foreground">0.5 ~ 2.0</span>
        </div>
      </div>

      <Button type="submit" disabled={submitting} className="w-full">
        {submitting ? "提交中..." : "提交生成"}
      </Button>
    </form>
  );
}
