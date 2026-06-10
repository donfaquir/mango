import { useState } from "react";
import { Trash2, Volume2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import type { AudioRole, ShotAudio } from "@/lib/bindings/commands";
import {
  useShotAudioList,
  useCreateShotAudio,
  useUpdateShotAudio,
  useDeleteShotAudio,
} from "@/hooks/useShotAudio";
import { useAssetList } from "@/hooks/useAssets";

const ROLE_LABELS: Record<AudioRole, string> = {
  voice: "配音",
  sfx: "音效",
  bgm: "BGM",
};

interface ShotAudioPanelProps {
  shotId: string;
  projectId: string;
}

export function ShotAudioPanel({ shotId, projectId }: ShotAudioPanelProps) {
  const { data: bindings = [] } = useShotAudioList(shotId);
  const audioAssets = useAssetList(projectId, "audio");

  const grouped = {
    voice: bindings.filter((b) => b.audio_role === "voice"),
    sfx: bindings.filter((b) => b.audio_role === "sfx"),
    bgm: bindings.filter((b) => b.audio_role === "bgm"),
  };

  return (
    <div className="space-y-3">
      <Label className="text-sm font-medium">音频绑定</Label>
      {(["voice", "sfx", "bgm"] as AudioRole[]).map((role) => (
        <RoleSection
          key={role}
          role={role}
          items={grouped[role]}
          shotId={shotId}
          audioAssets={audioAssets.data ?? []}
        />
      ))}
    </div>
  );
}

function RoleSection({
  role,
  items,
  shotId,
  audioAssets,
}: {
  role: AudioRole;
  items: ShotAudio[];
  shotId: string;
  audioAssets: { id: string; original_name: string }[];
}) {
  const create = useCreateShotAudio(shotId);
  const [picking, setPicking] = useState(false);
  const canAdd = role === "sfx" || items.length === 0;

  const available = audioAssets.filter(
    (a) => !items.some((b) => b.asset_id === a.id),
  );

  const handleSelect = async (assetId: string) => {
    setPicking(false);
    try {
      await create.mutateAsync({
        shot_id: shotId,
        asset_id: assetId,
        audio_role: role,
        volume: 1.0,
        offset_ms: 0,
      });
    } catch (err) {
      toast.error(`添加失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  return (
    <div className="rounded-md border p-2 space-y-1">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-muted-foreground">
          {ROLE_LABELS[role]}
        </span>
        {canAdd && !picking && (
          <Button
            variant="ghost"
            size="sm"
            className="h-6 text-xs"
            onClick={() => {
              if (available.length === 0) {
                toast.error("没有可用的音频素材，请先导入音频文件");
                return;
              }
              setPicking(true);
            }}
            disabled={create.isPending}
          >
            + 添加
          </Button>
        )}
      </div>
      {items.length === 0 && !picking && (
        <p className="text-xs text-muted-foreground">无</p>
      )}
      {items.map((item) => (
        <AudioBindingCard
          key={item.id}
          item={item}
          shotId={shotId}
          assetName={
            audioAssets.find((a) => a.id === item.asset_id)?.original_name ??
            "unknown"
          }
        />
      ))}
      {picking && (
        <div className="flex items-center gap-1">
          <Select onValueChange={handleSelect}>
            <SelectTrigger className="h-7 text-xs flex-1">
              <SelectValue placeholder="选择音频素材..." />
            </SelectTrigger>
            <SelectContent>
              {available.map((a) => (
                <SelectItem key={a.id} value={a.id}>
                  {a.original_name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            variant="ghost"
            size="sm"
            className="h-7 text-xs"
            onClick={() => setPicking(false)}
          >
            取消
          </Button>
        </div>
      )}
    </div>
  );
}

function AudioBindingCard({
  item,
  shotId,
  assetName,
}: {
  item: ShotAudio;
  shotId: string;
  assetName: string;
}) {
  const update = useUpdateShotAudio(shotId);
  const del = useDeleteShotAudio(shotId);

  return (
    <div className="flex items-center gap-2 text-xs">
      <Volume2 className="h-3 w-3 shrink-0 text-muted-foreground" />
      <span className="truncate flex-1" title={assetName}>
        {assetName}
      </span>
      <Slider
        className="w-20"
        min={0}
        max={200}
        step={5}
        value={[Math.round((item.volume ?? 1) * 100)]}
        onValueCommit={([v]) =>
          update.mutate({ id: item.id, input: { volume: v / 100 } })
        }
      />
      <span className="w-8 text-right text-muted-foreground">
        {Math.round((item.volume ?? 1) * 100)}%
      </span>
      <Input
        type="number"
        className="w-16 h-6 text-xs"
        value={item.offset_ms}
        onChange={(e) => {
          const ms = parseInt(e.target.value, 10);
          if (!isNaN(ms)) {
            update.mutate({ id: item.id, input: { offset_ms: ms } });
          }
        }}
        title="偏移 (ms)"
      />
      <Button
        variant="ghost"
        size="sm"
        className="h-6 w-6 p-0"
        onClick={() => del.mutate(item.id)}
      >
        <Trash2 className="h-3 w-3" />
      </Button>
    </div>
  );
}
