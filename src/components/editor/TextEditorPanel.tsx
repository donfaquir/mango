import { useMemo, useCallback } from "react";
import { AlignLeft, AlignCenter, AlignRight, Bold } from "lucide-react";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Slider } from "@/components/ui/slider";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { ColorPicker } from "./ColorPicker";
import { useMultiTrackStore } from "@/stores/multiTrackStore";

interface TextStyle {
  font_family?: string; font_size?: number; font_weight?: string;
  color?: string; outline_color?: string; outline_width?: number;
  shadow?: boolean; position_x?: number; position_y?: number; alignment?: string;
}
interface BubbleStyle { shape?: string; fill_color?: string; border_color?: string; }
interface TextParams { content: string; text_type: string; style: TextStyle; bubble?: BubbleStyle; }

const TEXT_TYPE_LABELS: Record<string, string> = { subtitle: "字幕", bubble: "对话气泡", fancy: "花字", onomatopoeia: "拟声词" };
const BUBBLE_SHAPES = [{ value: "rect", label: "矩形" }, { value: "oval", label: "椭圆" }, { value: "cloud", label: "云朵" }, { value: "shout", label: "呐喊" }] as const;

function parseTextParams(json: string): TextParams {
  try {
    const raw = JSON.parse(json) as Record<string, unknown>;
    return {
      content: typeof raw.content === "string" ? raw.content : "",
      text_type: typeof raw.text_type === "string" ? raw.text_type : "subtitle",
      style: (typeof raw.style === "object" && raw.style !== null ? raw.style : {}) as TextStyle,
      bubble: (typeof raw.bubble === "object" && raw.bubble !== null ? raw.bubble : undefined) as BubbleStyle | undefined,
    };
  } catch {
    return { content: "", text_type: "subtitle", style: {} };
  }
}

interface TextEditorPanelProps {
  itemId: string;
}

export function TextEditorPanel({ itemId }: TextEditorPanelProps) {
  const item = useMultiTrackStore((s) => s.items[itemId]);
  const updateItem = useMultiTrackStore((s) => s.updateItem);

  const params = useMemo(() => parseTextParams(item?.params_json ?? "{}"), [item?.params_json]);

  const update = useCallback(
    (patch: Partial<TextParams>) => {
      const merged = { ...params, ...patch };
      if (patch.style) merged.style = { ...params.style, ...patch.style };
      if (patch.bubble) merged.bubble = { ...params.bubble, ...patch.bubble };
      updateItem(itemId, {
        position_ms: null,
        duration_ms: null,
        in_point_ms: null,
        out_point_ms: null,
        params_json: JSON.stringify(merged),
      });
    },
    [params, itemId, updateItem],
  );

  if (!item) return null;
  const { style, bubble } = params;
  const isBubble = params.text_type === "bubble";

  return (
    <div className="w-64 border-l border-border bg-muted/10 overflow-y-auto p-3 space-y-3 text-sm shrink-0">
      <h3 className="font-medium text-xs text-muted-foreground uppercase tracking-wider">文字属性</h3>

      <div className="space-y-1">
        <Label className="text-xs">内容</Label>
        <Textarea
          className="h-16 text-xs resize-none"
          value={params.content}
          onChange={(e) => update({ content: e.target.value })}
        />
      </div>

      <div className="space-y-1">
        <Label className="text-xs">类型</Label>
        <span className="block text-xs text-muted-foreground">{TEXT_TYPE_LABELS[params.text_type] ?? params.text_type}</span>
      </div>

      <Separator />

      <div className="space-y-1">
        <Label className="text-xs">字号 ({style.font_size ?? 48})</Label>
        <Slider min={12} max={120} step={1} value={[style.font_size ?? 48]} onValueChange={([v]) => update({ style: { font_size: v } })} />
      </div>

      <ColorPicker label="字色" value={style.color ?? "#FFFFFF"} onChange={(c) => update({ style: { color: c } })} />

      <Button
        variant={style.font_weight === "bold" ? "default" : "outline"}
        size="sm"
        className="h-7 w-7 p-0"
        onClick={() => update({ style: { font_weight: style.font_weight === "bold" ? "normal" : "bold" } })}
      >
        <Bold className="size-3.5" />
      </Button>

      <Separator />

      <div className="space-y-1">
        <Label className="text-xs">描边宽度 ({style.outline_width ?? 0})</Label>
        <Slider min={0} max={5} step={1} value={[style.outline_width ?? 0]} onValueChange={([v]) => update({ style: { outline_width: v } })} />
      </div>
      <ColorPicker label="描边色" value={style.outline_color ?? "#000000"} onChange={(c) => update({ style: { outline_color: c } })} />

      <div className="flex items-center gap-2">
        <Label className="text-xs shrink-0">阴影</Label>
        <Button
          variant={style.shadow ? "default" : "outline"}
          size="sm"
          className="h-6 text-xs"
          onClick={() => update({ style: { shadow: !style.shadow } })}
        >
          {style.shadow ? "开" : "关"}
        </Button>
      </div>

      <Separator />

      <div className="space-y-1">
        <Label className="text-xs">对齐</Label>
        <div className="flex gap-1">
          {([["left", AlignLeft], ["center", AlignCenter], ["right", AlignRight]] as const).map(([align, Icon]) => (
            <Button
              key={align}
              variant={(style.alignment ?? "center") === align ? "default" : "outline"}
              size="sm"
              className="h-7 w-7 p-0"
              onClick={() => update({ style: { alignment: align } })}
            >
              <Icon className="size-3.5" />
            </Button>
          ))}
        </div>
      </div>

      <div className="space-y-1">
        <Label className="text-xs">X 位置 ({(style.position_x ?? 0.5).toFixed(2)})</Label>
        <Slider min={0} max={1} step={0.01} value={[style.position_x ?? 0.5]} onValueChange={([v]) => update({ style: { position_x: v } })} />
      </div>
      <div className="space-y-1">
        <Label className="text-xs">Y 位置 ({(style.position_y ?? 0.5).toFixed(2)})</Label>
        <Slider min={0} max={1} step={0.01} value={[style.position_y ?? 0.5]} onValueChange={([v]) => update({ style: { position_y: v } })} />
      </div>

      {isBubble && (
        <>
          <Separator />
          <h4 className="font-medium text-xs text-muted-foreground">气泡</h4>
          <div className="space-y-1">
            <Label className="text-xs">形状</Label>
            <Select value={bubble?.shape ?? "rect"} onValueChange={(v) => update({ bubble: { shape: v } })}>
              <SelectTrigger className="h-7 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {BUBBLE_SHAPES.map((s) => (
                  <SelectItem key={s.value} value={s.value}>{s.label}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <ColorPicker label="填充色" value={bubble?.fill_color ?? "#FFFFFF"} onChange={(c) => update({ bubble: { fill_color: c } })} />
          <ColorPicker label="边框色" value={bubble?.border_color ?? "#000000"} onChange={(c) => update({ bubble: { border_color: c } })} />
        </>
      )}
    </div>
  );
}
