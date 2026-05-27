import {
  useCharacterList,
} from "@/hooks/useCharacters";
import { useSceneList } from "@/hooks/useScenes";
import { usePropList } from "@/hooks/useProps";
import { useCostumeList } from "@/hooks/useCostumes";
import { ErrorAlert } from "@/components/common/ErrorAlert";
import type { SubjectKind } from "@/lib/subjectKind";
import { SubjectCard } from "./SubjectCard";
import { SubjectGridSkeleton } from "./SubjectGridSkeleton";
import { SubjectEmptyState } from "./SubjectEmptyState";
import type { SubjectListItem } from "./types";

interface SubjectGridProps {
  projectId: string;
  kind: SubjectKind;
  onCreate: () => void;
}

/**
 * Calls all four list hooks unconditionally each render (Rules of Hooks),
 * gates them via the existing `enabled: !!projectId` guard, and picks the
 * active query by kind. The three inactive queries stay idle and re-use any
 * cached data.
 */
export function SubjectGrid({ projectId, kind, onCreate }: SubjectGridProps) {
  const characters = useCharacterList(
    kind === "character" ? projectId : undefined,
  );
  const scenes = useSceneList(kind === "scene" ? projectId : undefined);
  const props = usePropList(kind === "prop" ? projectId : undefined);
  const costumes = useCostumeList(kind === "costume" ? projectId : undefined);

  const query = {
    character: characters,
    scene: scenes,
    prop: props,
    costume: costumes,
  }[kind];

  if (query.isLoading) return <SubjectGridSkeleton />;
  if (query.isError) {
    return <ErrorAlert error={query.error} onRetry={() => query.refetch()} />;
  }
  if (!query.data?.length) {
    return <SubjectEmptyState kind={kind} onCreate={onCreate} />;
  }

  return (
    <div className="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4">
      {(query.data as SubjectListItem[]).map((subj) => (
        <SubjectCard
          key={subj.id}
          subject={subj}
          kind={kind}
          projectId={projectId}
        />
      ))}
    </div>
  );
}
