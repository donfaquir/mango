import { Controller, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { toast } from "sonner";
import * as z from "zod";

import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { ReferenceImageDropzone } from "@/components/subjects/ReferenceImageDropzone";
import { useProject } from "@/hooks/useProjects";
import {
  useScene,
  useUpdateScene,
  useDeleteScene,
} from "@/hooks/useScenes";
import { QueryFallback, SubjectFormFooter } from "./SubjectFormShell";

const schema = z.object({
  name: z.string().trim().min(1, "名称必填").max(100),
  description: z.string().max(2000),
  environment_prompt: z.string().max(4000),
  reference_image_path: z.string().nullable(),
});
type FormValues = z.infer<typeof schema>;

export function SceneForm({
  projectId,
  subjectId,
}: {
  projectId: string;
  subjectId: string;
}) {
  const project = useProject(projectId);
  const scene = useScene(subjectId);
  const update = useUpdateScene(projectId);
  const del = useDeleteScene(projectId);

  const fallback = QueryFallback({
    isLoading: scene.isLoading,
    isError: scene.isError,
    error: scene.error,
    onRetry: () => scene.refetch(),
  });
  if (fallback) return fallback;
  if (!scene.data) return null;

  return (
    <Inner
      projectId={projectId}
      subjectId={subjectId}
      projectRoot={project.data?.root_path}
      currentName={scene.data.name}
      defaults={{
        name: scene.data.name,
        description: scene.data.description,
        environment_prompt: scene.data.environment_prompt,
        reference_image_path: scene.data.reference_image_path,
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
    <form onSubmit={onSubmit} className="flex flex-col gap-6 md:flex-row">
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
          <Label htmlFor="scene-name">名称 *</Label>
          <Input id="scene-name" {...form.register("name")} disabled={busy} />
          {form.formState.errors.name && (
            <p className="text-sm text-destructive">
              {form.formState.errors.name.message}
            </p>
          )}
        </div>
        <div className="space-y-2">
          <Label htmlFor="scene-desc">简介</Label>
          <Textarea
            id="scene-desc"
            rows={3}
            {...form.register("description")}
            disabled={busy}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="scene-env">环境描述</Label>
          <Textarea
            id="scene-env"
            rows={5}
            placeholder="如：午后阳光下的咖啡馆，木质桌椅..."
            {...form.register("environment_prompt")}
            disabled={busy}
          />
        </div>
        <SubjectFormFooter
          projectId={projectId}
          kind="scene"
          subjectId={subjectId}
          subjectName={currentName}
          submitting={form.formState.isSubmitting}
          deletePending={deletePending}
        />
      </div>
    </form>
  );
}
