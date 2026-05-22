import { Users, Trees, Package, Shirt } from "lucide-react";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/common/EmptyState";
import { SUBJECT_KIND_LABELS, type SubjectKind } from "@/lib/subjectKind";

const KIND_ICONS = {
  character: Users,
  scene: Trees,
  prop: Package,
  costume: Shirt,
} as const;

interface SubjectEmptyStateProps {
  kind: SubjectKind;
  onCreate: () => void;
}

export function SubjectEmptyState({ kind, onCreate }: SubjectEmptyStateProps) {
  const label = SUBJECT_KIND_LABELS[kind];
  return (
    <EmptyState
      icon={KIND_ICONS[kind]}
      title={`还没有${label}`}
      description={`创建你的第一个${label}，作为后续生图的参考`}
      action={<Button onClick={onCreate}>{`创建${label}`}</Button>}
    />
  );
}
