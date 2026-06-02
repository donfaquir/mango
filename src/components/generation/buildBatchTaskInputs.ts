import {
  commands,
  type Character,
  type CreateGenerationTaskInput,
  type Model,
  type Shot,
  type TaskKind,
} from "@/lib/bindings/commands";
import { resolveCharacterReferenceAssets } from "./resolveCharacterReferenceAssets";

export interface BatchBuildContext {
  projectId: string;
  accountId: string;
  providerId: string;
  taskType: TaskKind;
  shots: Shot[];
  models: Model[];
  characters: Character[];
}

export interface SkippedItem {
  shotId: string;
  shotLabel: string;
  reason: string;
}

export interface BatchBuildResult {
  inputs: CreateGenerationTaskInput[];
  skipped: SkippedItem[];
}

/**
 * Expand (shots × models) into ready-to-submit task inputs. Skips any
 * combination whose prompt is empty, or whose video-model reference media
 * cannot be resolved. The returned `skipped` list is surfaced verbatim to
 * the user before submitting so they can decide whether to proceed.
 *
 * Defaults for provider params come from `model.default_params_json` —
 * keeping the "smart per-shot params" logic on the frontend, where the
 * character→asset resolver already lives (see spec-26 §2 decision).
 */
export async function buildBatchTaskInputs(
  ctx: BatchBuildContext,
): Promise<BatchBuildResult> {
  const inputs: CreateGenerationTaskInput[] = [];
  const skipped: SkippedItem[] = [];

  // Resolve reference media per shot once — video models all share the same
  // shot.character_links, so we avoid N model_count × M shot_count IPC calls.
  const referenceMediaByShot = new Map<
    string,
    Awaited<ReturnType<typeof resolveCharacterReferenceAssets>>
  >();

  for (const shot of ctx.shots) {
    const shotLabel = labelFor(shot);
    const prompt =
      ctx.taskType === "image" ? shot.image_prompt : shot.video_prompt;
    if (!prompt || !prompt.trim()) {
      skipped.push({
        shotId: shot.id,
        shotLabel,
        reason: `${ctx.taskType === "image" ? "缺少图像 prompt" : "缺少视频 prompt"}`,
      });
      continue;
    }

    let resolvedMedia: typeof referenceMediaByShot extends Map<string, infer V>
      ? V | null
      : never = null;

    if (ctx.taskType === "video") {
      // Per-shot resolution: walk shot_character links so a wide-scene shot
      // doesn't accidentally pull in every project character as a reference.
      if (!referenceMediaByShot.has(shot.id)) {
        const linksResult = await commands.listShotLinks(shot.id);
        const characterIds =
          linksResult.status === "ok" ? linksResult.data.character_ids : [];
        const resolved = await resolveCharacterReferenceAssets(
          ctx.projectId,
          characterIds,
          ctx.characters,
        );
        referenceMediaByShot.set(shot.id, resolved);
      }
      resolvedMedia = referenceMediaByShot.get(shot.id) ?? null;
    }

    for (const model of ctx.models) {
      const defaults = tryParseDefaults(model.default_params_json);
      const params: Record<string, unknown> = { ...defaults, prompt };

      if (model.id === "happyhorse-1.0-r2v") {
        if (!resolvedMedia || resolvedMedia.media.length === 0) {
          skipped.push({
            shotId: shot.id,
            shotLabel,
            reason:
              resolvedMedia?.errorMessage ?? "happyhorse 缺少可用参考图",
          });
          continue;
        }
        params.media = resolvedMedia.media;
      }

      inputs.push({
        project_id: ctx.projectId,
        shot_id: shot.id,
        provider_id: ctx.providerId,
        model_id: model.id,
        account_id: ctx.accountId,
        task_type: ctx.taskType,
        params_json: JSON.stringify(params),
      });
    }
  }

  return { inputs, skipped };
}

function labelFor(shot: Shot): string {
  const summary = shot.summary?.trim();
  return `#${shot.order_index + 1}${summary ? ` ${summary.slice(0, 16)}` : ""}`;
}

function tryParseDefaults(raw: string | null): Record<string, unknown> {
  if (!raw) return {};
  try {
    const parsed = JSON.parse(raw);
    return typeof parsed === "object" && parsed !== null
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}
