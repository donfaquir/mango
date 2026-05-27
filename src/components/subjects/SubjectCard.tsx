import { Link } from "react-router-dom";
import { subjectDetailPath, type SubjectKind } from "@/lib/subjectKind";
import { SubjectThumbnail } from "./SubjectThumbnail";
import type { SubjectListItem } from "./types";

interface SubjectCardProps {
  subject: SubjectListItem;
  kind: SubjectKind;
  projectId: string;
}

export function SubjectCard({ subject, kind, projectId }: SubjectCardProps) {
  return (
    <Link
      to={subjectDetailPath(projectId, kind, subject.id)}
      className="flex flex-col gap-2 rounded-lg border p-3 text-left transition-colors hover:bg-accent"
    >
      <SubjectThumbnail
        projectId={projectId}
        relativePath={subject.reference_image_path}
      />
      <div className="truncate font-medium">{subject.name}</div>
      <div className="line-clamp-2 text-xs text-muted-foreground">
        {subject.description}
      </div>
    </Link>
  );
}
