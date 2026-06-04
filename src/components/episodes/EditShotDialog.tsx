import { useEffect, type FormEvent } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { toast } from "sonner";
import * as z from "zod";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { useUpdateShot } from "@/hooks/useShots";
import type { Shot, ShotStatus } from "@/lib/bindings/commands";

const STATUS_OPTIONS: { value: ShotStatus; label: string }[] = [
  { value: "draft", label: "草稿" },
  { value: "ready", label: "就绪" },
  { value: "generating", label: "生成中" },
  { value: "done", label: "完成" },
];

// duration_sec is optional and may be empty — keep it a string in the form
// and parse on submit so the user can clear the field.
const schema = z.object({
  summary: z.string().max(500),
  image_prompt: z.string().max(4000),
  video_prompt: z.string().max(4000),
  dialogue: z.string().max(2000),
  camera_angle: z.string().max(200),
  shot_type: z.string().max(200),
  mood: z.string().max(200),
  duration_sec: z
    .string()
    .trim()
    .refine((v) => v === "" || (!isNaN(Number(v)) && Number(v) >= 0), {
      message: "需要非负数或留空",
    }),
  status: z.enum(["draft", "ready", "generating", "done"]),
});
type FormValues = z.infer<typeof schema>;

interface Props {
  episodeId: string;
  shot: Shot;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function EditShotDialog({ episodeId, shot, open, onOpenChange }: Props) {
  const update = useUpdateShot(episodeId);
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: toFormValues(shot),
  });

  // Reset the form whenever we open a different shot (or re-open the dialog
  // after the parent state changed). Without this the form holds stale values
  // from the previous shot.
  useEffect(() => {
    if (open) form.reset(toFormValues(shot));
  }, [open, shot, form]);

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    await form.handleSubmit(async (v) => {
      try {
        await update.mutateAsync({
          id: shot.id,
          input: {
            summary: v.summary,
            image_prompt: v.image_prompt,
            video_prompt: v.video_prompt,
            dialogue: v.dialogue,
            camera_angle: v.camera_angle,
            shot_type: v.shot_type,
            mood: v.mood,
            duration_sec: v.duration_sec === "" ? null : Number(v.duration_sec),
            status: v.status,
          },
        });
        toast.success("已保存");
        onOpenChange(false);
      } catch (err) {
        toast.error(
          `保存失败：${err instanceof Error ? err.message : String(err)}`,
        );
      }
    })(e);
  };

  const busy = update.isPending;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-[640px]">
        <DialogHeader>
          <DialogTitle>编辑分镜</DialogTitle>
        </DialogHeader>
        <form onSubmit={onSubmit} className="space-y-4">
          <Field id="shot-summary" label="摘要">
            <Textarea
              id="shot-summary"
              rows={2}
              {...form.register("summary")}
              disabled={busy}
            />
          </Field>

          <Field
            id="shot-image-prompt"
            label="图像 prompt"
            hint="批量生成图像时使用"
          >
            <Textarea
              id="shot-image-prompt"
              rows={3}
              {...form.register("image_prompt")}
              disabled={busy}
            />
          </Field>

          <Field
            id="shot-video-prompt"
            label="视频 prompt"
            hint="批量生成视频时使用"
          >
            <Textarea
              id="shot-video-prompt"
              rows={3}
              {...form.register("video_prompt")}
              disabled={busy}
            />
          </Field>

          <Field id="shot-dialogue" label="对白">
            <Textarea
              id="shot-dialogue"
              rows={2}
              {...form.register("dialogue")}
              disabled={busy}
            />
          </Field>

          <div className="grid grid-cols-2 gap-3">
            <Field id="shot-camera" label="镜头角度">
              <Input
                id="shot-camera"
                {...form.register("camera_angle")}
                disabled={busy}
              />
            </Field>
            <Field id="shot-type" label="景别">
              <Input
                id="shot-type"
                {...form.register("shot_type")}
                disabled={busy}
              />
            </Field>
            <Field id="shot-mood" label="氛围">
              <Input
                id="shot-mood"
                {...form.register("mood")}
                disabled={busy}
              />
            </Field>
            <Field
              id="shot-duration"
              label="时长（秒）"
              error={form.formState.errors.duration_sec?.message}
            >
              <Input
                id="shot-duration"
                inputMode="decimal"
                placeholder="留空表示不限"
                {...form.register("duration_sec")}
                disabled={busy}
              />
            </Field>
          </div>

          <Field id="shot-status" label="状态">
            <Select
              value={form.watch("status")}
              onValueChange={(v) =>
                form.setValue("status", v as ShotStatus, { shouldDirty: true })
              }
            >
              <SelectTrigger id="shot-status" disabled={busy}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {STATUS_OPTIONS.map((o) => (
                  <SelectItem key={o.value} value={o.value}>
                    {o.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={busy}
            >
              取消
            </Button>
            <Button type="submit" disabled={busy}>
              {busy ? "保存中..." : "保存"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function Field({
  id,
  label,
  hint,
  error,
  children,
}: {
  id: string;
  label: string;
  hint?: string;
  error?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <Label htmlFor={id}>{label}</Label>
      {children}
      {hint && !error && (
        <p className="text-xs text-muted-foreground">{hint}</p>
      )}
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}

function toFormValues(shot: Shot): FormValues {
  return {
    summary: shot.summary,
    image_prompt: shot.image_prompt,
    video_prompt: shot.video_prompt,
    dialogue: shot.dialogue,
    camera_angle: shot.camera_angle,
    shot_type: shot.shot_type,
    mood: shot.mood,
    duration_sec:
      shot.duration_sec === null ? "" : String(shot.duration_sec),
    status: shot.status,
  };
}
