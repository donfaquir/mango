import { Controller, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { toast } from "sonner";
import * as z from "zod";

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
import { ReferenceImageDropzone } from "@/components/subjects/ReferenceImageDropzone";
import { useProject } from "@/hooks/useProjects";
import {
  useCharacter,
  useUpdateCharacter,
  useDeleteCharacter,
} from "@/hooks/useCharacters";
import { QueryFallback, SubjectFormFooter } from "./SubjectFormShell";

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

const schema = z.object({
  name: z.string().trim().min(1, "名称必填").max(100),
  description: z.string().max(2000),
  appearance_prompt: z.string().max(4000),
  reference_image_path: z.string().nullable(),
  voice_id: z.string().nullable(),
});
type FormValues = z.infer<typeof schema>;

export function CharacterForm({
  projectId,
  subjectId,
}: {
  projectId: string;
  subjectId: string;
}) {
  const project = useProject(projectId);
  const character = useCharacter(subjectId);
  const update = useUpdateCharacter(projectId);
  const del = useDeleteCharacter(projectId);

  const fallback = QueryFallback({
    isLoading: character.isLoading,
    isError: character.isError,
    error: character.error,
    onRetry: () => character.refetch(),
  });
  if (fallback) return fallback;
  if (!character.data) return null;

  return (
    <Inner
      projectId={projectId}
      subjectId={subjectId}
      projectRoot={project.data?.root_path}
      currentName={character.data.name}
      defaults={{
        name: character.data.name,
        description: character.data.description,
        appearance_prompt: character.data.appearance_prompt,
        reference_image_path: character.data.reference_image_path,
        voice_id: character.data.voice_id,
      }}
      submit={(v) => update.mutateAsync({ id: subjectId, input: v })}
      deletePending={del.isPending}
    />
  );
}

function Inner({
  projectId,
  subjectId,
  projectRoot,
  currentName,
  defaults,
  submit,
  deletePending,
}: {
  projectId: string;
  subjectId: string;
  projectRoot: string | undefined;
  currentName: string;
  defaults: FormValues;
  submit: (v: FormValues) => Promise<unknown>;
  deletePending: boolean;
}) {
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: defaults,
  });

  const onSubmit = form.handleSubmit(async (v) => {
    try {
      await submit(v);
      toast.success("已保存");
    } catch (err) {
      toast.error(
        `保存失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  });
  const busy = form.formState.isSubmitting || deletePending;

  return (
    <form
      onSubmit={onSubmit}
      className="flex flex-col gap-6 md:flex-row"
    >
      <Controller
        control={form.control}
        name="reference_image_path"
        render={({ field }) => (
          <ReferenceImageDropzone
            projectId={projectId}
            projectRoot={projectRoot}
            currentRelativePath={field.value}
            onChange={field.onChange}
            disabled={busy}
          />
        )}
      />
      <div className="flex flex-1 flex-col gap-4">
        <div className="space-y-2">
          <Label htmlFor="char-name">名称 *</Label>
          <Input
            id="char-name"
            {...form.register("name")}
            disabled={busy}
          />
          {form.formState.errors.name && (
            <p className="text-sm text-destructive">
              {form.formState.errors.name.message}
            </p>
          )}
        </div>
        <div className="space-y-2">
          <Label htmlFor="char-desc">简介</Label>
          <Textarea
            id="char-desc"
            rows={3}
            {...form.register("description")}
            disabled={busy}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="char-appearance">外貌描述</Label>
          <Textarea
            id="char-appearance"
            rows={5}
            placeholder="如：黑色短发，琥珀色眼睛..."
            {...form.register("appearance_prompt")}
            disabled={busy}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="char-voice">配音音色</Label>
          <Controller
            control={form.control}
            name="voice_id"
            render={({ field }) => (
              <Select
                value={field.value ?? "__none__"}
                onValueChange={(v) => field.onChange(v === "__none__" ? null : v)}
                disabled={busy}
              >
                <SelectTrigger id="char-voice">
                  <SelectValue placeholder="未设置" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="__none__">未设置</SelectItem>
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
            )}
          />
        </div>
        <SubjectFormFooter
          projectId={projectId}
          kind="character"
          subjectId={subjectId}
          subjectName={currentName}
          submitting={form.formState.isSubmitting}
          deletePending={deletePending}
        />
      </div>
    </form>
  );
}
