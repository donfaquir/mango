import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useDeleteCharacter } from "@/hooks/useCharacters";
import { useDeleteScene } from "@/hooks/useScenes";
import { useDeleteProp } from "@/hooks/useProps";
import {
  useCostumeListByCharacter,
  useDeleteCostume,
} from "@/hooks/useCostumes";
import {
  SUBJECT_KIND_LABELS,
  subjectListPath,
  type SubjectKind,
} from "@/lib/subjectKind";

interface Props {
  projectId: string;
  kind: SubjectKind;
  subjectId: string;
  subjectName: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function DeleteSubjectDialog({
  projectId,
  kind,
  subjectId,
  subjectName,
  open,
  onOpenChange,
}: Props) {
  const navigate = useNavigate();
  const deleteCharacter = useDeleteCharacter(projectId);
  const deleteScene = useDeleteScene(projectId);
  const deleteProp = useDeleteProp(projectId);
  const deleteCostume = useDeleteCostume(projectId);

  const costumesByCharacter = useCostumeListByCharacter(
    kind === "character" && open ? projectId : undefined,
    kind === "character" && open ? subjectId : undefined,
  );

  const mutation = {
    character: deleteCharacter,
    scene: deleteScene,
    prop: deleteProp,
    costume: deleteCostume,
  }[kind];

  const label = SUBJECT_KIND_LABELS[kind];
  const costumeCount = costumesByCharacter.data?.length ?? 0;
  const costumesLoading =
    kind === "character" && costumesByCharacter.isLoading;

  const handleConfirm = async () => {
    try {
      await mutation.mutateAsync(subjectId);
      toast.success(`${label}已删除`);
      onOpenChange(false);
      navigate(subjectListPath(projectId, kind));
    } catch (err) {
      toast.error(
        `删除失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[420px]">
        <DialogHeader>
          <DialogTitle>{`删除${label}`}</DialogTitle>
          <DialogDescription asChild>
            <div className="space-y-2">
              <p>
                确认删除「{subjectName}」吗？此操作不可撤销。
              </p>
              {kind === "character" &&
                (costumesLoading ? (
                  <p className="text-muted-foreground">正在统计关联服装…</p>
                ) : costumeCount > 0 ? (
                  <p className="text-destructive">
                    此角色有 {costumeCount} 件服装将被一并删除
                  </p>
                ) : (
                  <p className="text-muted-foreground">无服装将受影响</p>
                ))}
            </div>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={mutation.isPending}
          >
            取消
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={handleConfirm}
            disabled={mutation.isPending || costumesLoading}
          >
            {mutation.isPending ? "删除中..." : "确认删除"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
