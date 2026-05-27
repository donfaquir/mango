import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
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
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useCreateCharacter, useCharacterList } from "@/hooks/useCharacters";
import { useCreateScene } from "@/hooks/useScenes";
import { useCreateProp } from "@/hooks/useProps";
import { useCreateCostume } from "@/hooks/useCostumes";
import {
  SUBJECT_KIND_LABELS,
  subjectDetailPath,
  type SubjectKind,
} from "@/lib/subjectKind";

interface Props {
  projectId: string;
  kind: SubjectKind;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CreateSubjectDialog({
  projectId,
  kind,
  open,
  onOpenChange,
}: Props) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [characterId, setCharacterId] = useState("");
  const navigate = useNavigate();

  const characters = useCharacterList(
    kind === "costume" ? projectId : undefined,
  );
  const createCharacter = useCreateCharacter();
  const createScene = useCreateScene();
  const createProp = useCreateProp();
  const createCostume = useCreateCostume();

  const mutation = {
    character: createCharacter,
    scene: createScene,
    prop: createProp,
    costume: createCostume,
  }[kind];

  const label = SUBJECT_KIND_LABELS[kind];

  const reset = () => {
    setName("");
    setDescription("");
    setCharacterId("");
    mutation.reset();
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;
    if (kind === "costume" && !characterId) return;

    const base = {
      project_id: projectId,
      name: trimmed,
      description: description.trim() || null,
    };
    let created;
    if (kind === "character") {
      created = await createCharacter.mutateAsync(base);
    } else if (kind === "scene") {
      created = await createScene.mutateAsync(base);
    } else if (kind === "prop") {
      created = await createProp.mutateAsync(base);
    } else {
      created = await createCostume.mutateAsync({
        ...base,
        character_id: characterId,
      });
    }
    reset();
    onOpenChange(false);
    navigate(subjectDetailPath(projectId, kind, created.id));
  };

  const disabled =
    mutation.isPending ||
    !name.trim() ||
    (kind === "costume" && !characterId);

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) reset();
        onOpenChange(next);
      }}
    >
      <DialogContent className="sm:max-w-[460px]">
        <DialogHeader>
          <DialogTitle>{`新建${label}`}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="subject-name">名称 *</Label>
            <Input
              id="subject-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              autoFocus
              maxLength={100}
            />
          </div>
          {kind === "costume" && (
            <div className="space-y-2">
              <Label>所属角色 *</Label>
              <Select value={characterId} onValueChange={setCharacterId}>
                <SelectTrigger>
                  <SelectValue placeholder="选择角色" />
                </SelectTrigger>
                <SelectContent>
                  {characters.data?.map((c) => (
                    <SelectItem key={c.id} value={c.id}>
                      {c.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {!characters.data?.length && (
                <p className="text-xs text-muted-foreground">
                  需先创建角色才能添加服装
                </p>
              )}
            </div>
          )}
          <div className="space-y-2">
            <Label htmlFor="subject-description">简介</Label>
            <Textarea
              id="subject-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={3}
              maxLength={2000}
            />
          </div>
          {mutation.error && (
            <p className="text-sm text-destructive">
              创建失败：
              {mutation.error instanceof Error
                ? mutation.error.message
                : String(mutation.error)}
            </p>
          )}
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              取消
            </Button>
            <Button type="submit" disabled={disabled}>
              {mutation.isPending ? "创建中..." : "创建"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
