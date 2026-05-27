import { useState } from "react";
import { Navigate, useParams } from "react-router-dom";
import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  isValidSubjectKind,
  SUBJECT_KIND_LABELS,
} from "@/lib/subjectKind";
import { SubjectTabs } from "@/components/subjects/SubjectTabs";
import { SubjectGrid } from "@/components/subjects/SubjectGrid";
import { CreateSubjectDialog } from "@/components/subjects/CreateSubjectDialog";

export default function SubjectLibraryPage() {
  const { projectId, kind } = useParams<{
    projectId: string;
    kind: string;
  }>();
  const [createOpen, setCreateOpen] = useState(false);

  if (!projectId) return <Navigate to="/" replace />;
  if (!isValidSubjectKind(kind)) {
    return (
      <Navigate to={`/project/${projectId}/subjects/character`} replace />
    );
  }

  const label = SUBJECT_KIND_LABELS[kind];

  return (
    <div className="flex flex-col gap-4">
      <header className="flex items-center justify-between">
        <h2 className="text-xl font-semibold">主体库 / {label}</h2>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus className="mr-1 h-4 w-4" />
          新建{label}
        </Button>
      </header>
      <SubjectTabs projectId={projectId} activeKind={kind} />
      <SubjectGrid
        projectId={projectId}
        kind={kind}
        onCreate={() => setCreateOpen(true)}
      />
      <CreateSubjectDialog
        projectId={projectId}
        kind={kind}
        open={createOpen}
        onOpenChange={setCreateOpen}
      />
    </div>
  );
}
