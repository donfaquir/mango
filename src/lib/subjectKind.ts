export type SubjectKind = "character" | "scene" | "prop" | "costume";

export const SUBJECT_KINDS: readonly SubjectKind[] = [
  "character",
  "scene",
  "prop",
  "costume",
] as const;

export const SUBJECT_KIND_LABELS: Record<SubjectKind, string> = {
  character: "角色",
  scene: "场景",
  prop: "道具",
  costume: "服装",
};

export function isValidSubjectKind(s: string | undefined): s is SubjectKind {
  return !!s && (SUBJECT_KINDS as readonly string[]).includes(s);
}

export function subjectListPath(projectId: string, kind: SubjectKind): string {
  return `/project/${projectId}/subjects/${kind}`;
}

export function subjectDetailPath(
  projectId: string,
  kind: SubjectKind,
  id: string,
): string {
  return `/project/${projectId}/subjects/${kind}/${id}`;
}
