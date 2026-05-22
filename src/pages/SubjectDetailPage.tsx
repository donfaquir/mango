import { Navigate, useParams } from "react-router-dom";
import { isValidSubjectKind, subjectListPath } from "@/lib/subjectKind";
import { CharacterForm } from "@/components/subjects/forms/CharacterForm";
import { SceneForm } from "@/components/subjects/forms/SceneForm";
import { PropForm } from "@/components/subjects/forms/PropForm";
import { CostumeForm } from "@/components/subjects/forms/CostumeForm";

export default function SubjectDetailPage() {
  const { projectId, kind, subjectId } = useParams<{
    projectId: string;
    kind: string;
    subjectId: string;
  }>();

  if (!projectId) return <Navigate to="/" replace />;
  if (!isValidSubjectKind(kind)) {
    return (
      <Navigate to={`/project/${projectId}/subjects/character`} replace />
    );
  }
  if (!subjectId) {
    return <Navigate to={subjectListPath(projectId, kind)} replace />;
  }

  switch (kind) {
    case "character":
      return <CharacterForm projectId={projectId} subjectId={subjectId} />;
    case "scene":
      return <SceneForm projectId={projectId} subjectId={subjectId} />;
    case "prop":
      return <PropForm projectId={projectId} subjectId={subjectId} />;
    case "costume":
      return <CostumeForm projectId={projectId} subjectId={subjectId} />;
  }
}
