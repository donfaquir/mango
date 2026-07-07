import { useQuery } from "@tanstack/react-query";
import { Label } from "@/components/ui/label";
import { Slider } from "@/components/ui/slider";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { commands, type RenderConfig, type PlatformPresetInfo } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

const CODECS = [
  { value: "libx264", label: "H.264" },
  { value: "libx265", label: "H.265" },
];

const PRESETS = [
  { value: "ultrafast", label: "极快" },
  { value: "fast", label: "快速" },
  { value: "medium", label: "中等" },
  { value: "slow", label: "慢速" },
];

const RESOLUTIONS: { value: string; label: string; scale: number }[] = [
  { value: "720", label: "720p", scale: 720 },
  { value: "1080", label: "1080p", scale: 1080 },
  { value: "1440", label: "1440p", scale: 1440 },
];

const FPS_OPTIONS = [
  { value: "24", label: "24 fps" },
  { value: "30", label: "30 fps" },
  { value: "60", label: "60 fps" },
  { value: "original", label: "原始" },
];

const FIT_MODES = [
  { value: "crop", label: "裁剪" },
  { value: "pad", label: "填充" },
  { value: "stretch", label: "拉伸" },
];

interface ExportConfigFormProps {
  config: RenderConfig;
  onChange: (config: RenderConfig) => void;
  disabled: boolean;
}

export function ExportConfigForm({ config, onChange, disabled }: ExportConfigFormProps) {
  const { data: platforms = [] } = useQuery({
    queryKey: ["platform-presets"],
    queryFn: () => unwrap(commands.listPlatformPresets()),
    staleTime: Infinity,
  });

  const { data: aspectRatios = [] } = useQuery({
    queryKey: ["aspect-ratio-presets"],
    queryFn: () => unwrap(commands.listAspectRatioPresets()),
    staleTime: Infinity,
  });

  const applyPlatform = (p: PlatformPresetInfo) => {
    onChange({
      ...config,
      video_codec: p.codec,
      preset: p.preset,
      crf: p.crf,
      output_width: p.width,
      output_height: p.height,
    });
  };

  const currentRes = config.output_height
    ? RESOLUTIONS.find((r) => r.scale === config.output_height)?.value ?? "custom"
    : "original";

  const currentFps = config.output_fps
    ? FPS_OPTIONS.find((f) => f.value === String(config.output_fps))?.value ?? "custom"
    : "original";

  return (
    <div className="flex flex-col gap-3">
      <div className="space-y-1.5">
        <Label className="text-xs">平台预设</Label>
        <div className="flex flex-wrap gap-1">
          {platforms.map((p) => (
            <Button
              key={p.id}
              variant="outline"
              size="sm"
              className="h-6 text-xs"
              disabled={disabled}
              onClick={() => applyPlatform(p)}
            >
              {p.label}
            </Button>
          ))}
        </div>
      </div>

      <div className="space-y-1.5">
        <Label className="text-xs">宽高比</Label>
        <div className="flex flex-wrap gap-1">
          {aspectRatios.map((ar) => (
            <Button
              key={ar.id}
              variant="outline"
              size="sm"
              className="h-6 text-xs"
              disabled={disabled}
              onClick={() => onChange({ ...config, output_width: ar.width, output_height: ar.height })}
            >
              {ar.label}
            </Button>
          ))}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-2">
        <div className="space-y-1">
          <Label className="text-xs">适配模式</Label>
          <Select
            value={config.fit_mode ?? "stretch"}
            onValueChange={(v) => onChange({ ...config, fit_mode: v })}
            disabled={disabled}
          >
            <SelectTrigger className="h-7 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {FIT_MODES.map((m) => (
                <SelectItem key={m.value} value={m.value}>{m.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-1">
          <Label className="text-xs">编码器</Label>
          <Select
            value={config.video_codec}
            onValueChange={(v) => onChange({ ...config, video_codec: v })}
            disabled={disabled}
          >
            <SelectTrigger className="h-7 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {CODECS.map((c) => (
                <SelectItem key={c.value} value={c.value}>{c.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-1">
          <Label className="text-xs">编码速度</Label>
          <Select
            value={config.preset}
            onValueChange={(v) => onChange({ ...config, preset: v })}
            disabled={disabled}
          >
            <SelectTrigger className="h-7 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {PRESETS.map((p) => (
                <SelectItem key={p.value} value={p.value}>{p.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-1">
          <Label className="text-xs">分辨率</Label>
          <Select
            value={currentRes}
            onValueChange={(v) => {
              if (v === "original") {
                onChange({ ...config, output_width: null, output_height: null });
              } else {
                const r = RESOLUTIONS.find((r) => r.value === v);
                if (r) {
                  const ratio = config.output_width && config.output_height
                    ? config.output_width / config.output_height
                    : 16 / 9;
                  const h = r.scale;
                  const w = Math.round(h * ratio);
                  onChange({ ...config, output_width: w, output_height: h });
                }
              }
            }}
            disabled={disabled}
          >
            <SelectTrigger className="h-7 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {RESOLUTIONS.map((r) => (
                <SelectItem key={r.value} value={r.value}>{r.label}</SelectItem>
              ))}
              <SelectItem value="original">原始</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-1">
          <Label className="text-xs">帧率</Label>
          <Select
            value={currentFps}
            onValueChange={(v) => {
              onChange({ ...config, output_fps: v === "original" ? null : Number(v) });
            }}
            disabled={disabled}
          >
            <SelectTrigger className="h-7 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {FPS_OPTIONS.map((f) => (
                <SelectItem key={f.value} value={f.value}>{f.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      <div className="space-y-1">
        <div className="flex items-center justify-between">
          <Label className="text-xs">画质 (CRF)</Label>
          <span className="text-xs text-muted-foreground font-mono">{config.crf}</span>
        </div>
        <Slider
          min={0}
          max={51}
          step={1}
          value={[config.crf]}
          onValueChange={([v]) => onChange({ ...config, crf: v })}
          disabled={disabled}
        />
        <div className="flex justify-between text-[10px] text-muted-foreground">
          <span>高质量</span>
          <span>低质量</span>
        </div>
      </div>
    </div>
  );
}
