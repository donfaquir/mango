import { Controller, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { toast } from "sonner";
import * as z from "zod";

import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { ReferenceImageDropzone } from "@/components/subjects/ReferenceImageDropzone";
import { useProject } from "@/hooks/useProjects";
import { useProp, useUpdateProp, useDeleteProp } from "@/hooks/useProps";
import { QueryFallback, SubjectFormFooter } from "./SubjectFormShell";

const schema = z.object({
  name: z.string().trim().min(1, "名称必填").max(100),
  description: z.string().max(2000),
  reference_image_path: z.string().nullable(),
});
type FormValues = z.infer<typeof schema>;

export function PropForm({
  projectId,
  subjectId,
}: {
  projectId: string;
  subjectId: string;
}) {
  const project = useProject(projectId);
  const prop = useProp(subjectId);
  const update = useUpdateProp(projectId);
  const del = useDeleteProp(projectId);

  const fallback = QueryFallback({
    isLoading: prop.isLoading,
    isError: prop.isError,
    error: prop.error,
    onRetry: () => prop.refetch(),
  });
  if (fallback) return fallback;
  if (!prop.data) return null;

  return (
    <Inner
      projectId={projectId}
      subjectId={subjectId}
      projectRoot={project.data?.root_path}
      currentName={prop.data.name}
      defaults={{
        name: prop.data.name,
        description: prop.data.description,
        reference_image_path: prop.data.reference_image_path,
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
          <Label htmlFor="prop-name">名称 *</Label>
          <Input id="prop-name" {...form.register("name")} disabled={busy} />
          {form.formState.errors.name && (
            <p className="text-sm text-destructive">
              {form.formState.errors.name.message}
            </p>
          )}
        </div>
        <div className="space-y-2">
          <Label htmlFor="prop-desc">简介</Label>
          <Textarea
            id="prop-desc"
            rows={4}
            {...form.register("description")}
            disabled={busy}
          />
        </div>
        <SubjectFormFooter
          projectId={projectId}
          kind="prop"
          subjectId={subjectId}
          subjectName={currentName}
          submitting={form.formState.isSubmitting}
          deletePending={deletePending}
        />
      </div>
    </form>
  );
}
