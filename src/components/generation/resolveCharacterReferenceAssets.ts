import { commands, type Character } from "@/lib/bindings/commands";

export interface ResolvedReferenceMedia {
  asset_id: string;
  type: "reference_image";
}

export interface CharacterReferenceResolution {
  /** Resolved media entries, ready to embed into params_json. */
  media: ResolvedReferenceMedia[];
  /**
   * Human-readable summary of any subjects that could not be resolved. When
   * `media` is empty this is the error message; when `media` is non-empty
   * but some subjects were skipped, treat it as a warning.
   */
  errorMessage: string | null;
}

/**
 * Map character IDs to the asset rows behind their reference images.
 *
 * Characters store `reference_image_path` as a project-relative path that
 * points to an asset already imported via `import_asset` (the
 * ReferenceImageDropzone runs the import on upload). The runner, however,
 * dereferences `media[].asset_id` against the asset table — so the submit
 * payload has to carry asset IDs, not character IDs.
 */
export async function resolveCharacterReferenceAssets(
  projectId: string,
  subjectIds: string[],
  characters: Character[],
): Promise<CharacterReferenceResolution> {
  const charactersById = new Map(characters.map((c) => [c.id, c]));
  const skipped: string[] = [];
  const media: ResolvedReferenceMedia[] = [];

  for (const subjectId of subjectIds) {
    const character = charactersById.get(subjectId);
    if (!character) {
      skipped.push(`未知主体 ${subjectId.slice(0, 8)}`);
      continue;
    }
    if (!character.reference_image_path) {
      skipped.push(`${character.name}（无参考图）`);
      continue;
    }

    const result = await commands.findAssetByPath(
      projectId,
      character.reference_image_path,
    );
    if (result.status !== "ok") {
      skipped.push(`${character.name}（查询失败：${result.error.message}）`);
      continue;
    }
    if (result.data === null) {
      skipped.push(`${character.name}（参考图不在素材库）`);
      continue;
    }

    media.push({ asset_id: result.data.id, type: "reference_image" });
  }

  const errorMessage =
    skipped.length === 0
      ? null
      : `以下主体未能用作参考图：${skipped.join("、")}`;

  return { media, errorMessage };
}
