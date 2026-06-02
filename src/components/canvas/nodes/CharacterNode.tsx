import type { NodeProps } from "@xyflow/react";
import { User } from "lucide-react";
import { useCharacter } from "@/hooks/useCharacters";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";
import type { CharacterNode as CharacterNodeType } from "./types";
import { NodeShell } from "./shared/NodeShell";
import { useCurrentProjectRoot } from "./shared/useCurrentProjectRoot";

export function CharacterNode({
  data,
  selected,
}: NodeProps<CharacterNodeType>) {
  const { data: character } = useCharacter(data.characterId);
  const projectRoot = useCurrentProjectRoot();
  const refUrl = useResolvedAssetUrl(
    projectRoot,
    character?.reference_image_path ?? null,
  );

  return (
    <NodeShell selected={selected} width={200}>
      <div className="flex items-center gap-2 border-b px-3 py-2 text-sm font-medium">
        <User className="size-4 text-muted-foreground" />
        <span className="truncate">{character?.name ?? "角色"}</span>
      </div>
      <div className="flex items-center justify-center bg-muted/40 p-2">
        {refUrl ? (
          <img
            src={refUrl}
            alt={character?.name ?? ""}
            className="h-24 w-full rounded object-cover"
          />
        ) : (
          <div className="flex h-24 w-full items-center justify-center rounded bg-muted text-muted-foreground">
            <User className="size-8 opacity-40" />
          </div>
        )}
      </div>
    </NodeShell>
  );
}
