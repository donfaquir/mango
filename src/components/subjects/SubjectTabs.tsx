import { NavLink } from "react-router-dom";
import {
  SUBJECT_KINDS,
  SUBJECT_KIND_LABELS,
  subjectListPath,
  type SubjectKind,
} from "@/lib/subjectKind";
import { cn } from "@/lib/utils";

interface SubjectTabsProps {
  projectId: string;
  activeKind: SubjectKind;
}

export function SubjectTabs({ projectId, activeKind }: SubjectTabsProps) {
  return (
    <div
      role="tablist"
      aria-label="主体分类"
      className="inline-flex h-9 items-center justify-start gap-1 rounded-lg bg-muted p-1 text-muted-foreground w-fit"
    >
      {SUBJECT_KINDS.map((kind) => (
        <NavLink
          key={kind}
          to={subjectListPath(projectId, kind)}
          role="tab"
          aria-selected={activeKind === kind}
          className={cn(
            "inline-flex items-center justify-center rounded-md px-3 py-1 text-sm font-medium transition-all",
            activeKind === kind
              ? "bg-background text-foreground shadow"
              : "hover:text-foreground",
          )}
        >
          {SUBJECT_KIND_LABELS[kind]}
        </NavLink>
      ))}
    </div>
  );
}
