import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { ErrorAlert } from "@/components/common/ErrorAlert";
import { DeleteSubjectDialog } from "@/components/subjects/DeleteSubjectDialog";
import { subjectListPath, type SubjectKind } from "@/lib/subjectKind";

interface FormButtonsProps {
  projectId: string;
  kind: SubjectKind;
  subjectId: string;
  subjectName: string;
  submitting: boolean;
  deletePending: boolean;
}

/**
 * Three-button footer ("返回 / 删除 / 保存") + the wired-up delete confirm
 * dialog. Each form renders this inside its own `<form>` so the submit
 * button triggers the form's native submit event.
 */
export function SubjectFormFooter({
  projectId,
  kind,
  subjectId,
  subjectName,
  submitting,
  deletePending,
}: FormButtonsProps) {
  const navigate = useNavigate();
  const [deleteOpen, setDeleteOpen] = useState(false);
  const busy = submitting || deletePending;

  return (
    <>
      <div className="flex justify-between">
        <Button
          type="button"
          variant="outline"
          onClick={() => navigate(subjectListPath(projectId, kind))}
          disabled={busy}
        >
          返回主体库
        </Button>
        <div className="flex gap-2">
          <Button
            type="button"
            variant="destructive"
            onClick={() => setDeleteOpen(true)}
            disabled={busy}
          >
            删除
          </Button>
          <Button type="submit" disabled={busy}>
            {submitting ? "保存中..." : "保存"}
          </Button>
        </div>
      </div>
      <DeleteSubjectDialog
        projectId={projectId}
        kind={kind}
        subjectId={subjectId}
        subjectName={subjectName}
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
      />
    </>
  );
}

interface QueryFallbackProps {
  isLoading: boolean;
  isError: boolean;
  error: unknown;
  onRetry: () => void;
}

export function QueryFallback({
  isLoading,
  isError,
  error,
  onRetry,
}: QueryFallbackProps) {
  if (isLoading) return <p className="text-muted-foreground">加载中...</p>;
  if (isError) return <ErrorAlert error={error} onRetry={onRetry} />;
  return null;
}
