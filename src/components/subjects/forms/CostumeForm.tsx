import { Controller, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { toast } from "sonner";
import * as z from "zod";

import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ReferenceImageDropzone } from "@/components/subjects/ReferenceImageDropzone";
import { useProject } from "@/hooks/useProjects";
import { useCharacterList } from "@/hooks/useCharacters";
import {
  useCostume,
  useUpdateCostume,
  useDeleteCostume,
} from "@/hooks/useCostumes";
import { QueryFallback, SubjectFormFooter } from "./SubjectFormShell";

const schema = z.object({
  name: z.string().trim().min(1, "名称必填").max(100),
  description: z.string().max(2000),
  character_id: z.string().min(1, "必须选择角色"),
  reference_image_path: z.string().nullable(),
});
type FormValues = z.infer<typeof schema>;

export function CostumeForm({
  projectId,
  subjectId,
}: {
  projectId: string;
  subjectId: string;
}) {
  const project = useProject(projectId);
  const costume = useCostume(subjectId);
  const characters = useCharacterList(projectId);
  const update = useUpdateCostume(projectId);
  const del = useDeleteCostume(projectId);

  const fallback = QueryFallback({
    isLoading: costume.isLoading,
    isError: costume.isError,
    error: costume.error,
    onRetry: () => costume.refetch(),
  });
  if (fallback) return fallback;
  if (!costume.data) return null;

  return (
    <Inner
      projectId={projectId}
      subjectId={subjectId}
      projectRoot={project.data?.root_path}
      currentName={costume.data.name}
      characters={characters.data ?? []}
      defaults={{
        name: costume.data.name,
        description: costume.data.description,
        character_id: costume.data.character_id,
        reference_image_path: costume.data.reference_image_path,
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
  characters,
  defaults,
  submit,
  deletePending,
}: {
  projectId: string;
  subjectId: string;
  projectRoot: string | undefined;
  currentName: string;
  characters: { id: string; name: string }[];
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
          <Label htmlFor="costume-name">名称 *</Label>
          <Input
            id="costume-name"
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
          <Label>所属角色 *</Label>
          <Controller
            control={form.control}
            name="character_id"
            render={({ field }) => (
              <Select
                value={field.value}
                onValueChange={field.onChange}
                disabled={busy}
              >
                <SelectTrigger>
                  <SelectValue placeholder="选择角色" />
                </SelectTrigger>
                <SelectContent>
                  {characters.map((c) => (
                    <SelectItem key={c.id} value={c.id}>
                      {c.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="costume-desc">简介</Label>
          <Textarea
            id="costume-desc"
            rows={4}
            {...form.register("description")}
            disabled={busy}
          />
        </div>
        <SubjectFormFooter
          projectId={projectId}
          kind="costume"
          subjectId={subjectId}
          subjectName={currentName}
          submitting={form.formState.isSubmitting}
          deletePending={deletePending}
        />
      </div>
    </form>
  );
}
