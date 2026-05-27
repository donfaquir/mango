import { convertFileSrc } from "@tauri-apps/api/core";
import { join } from "@tauri-apps/api/path";

/**
 * Resolve a project-relative asset path (e.g. `assets/<id>.png`) to a URL the
 * webview can load via `<img src>`. The Rust side must have registered
 * `project_root` with the asset-protocol scope first — `import_asset` and
 * `register_project_asset_scope` both do this.
 */
export async function resolveAssetUrl(
  projectRoot: string,
  relativePath: string,
): Promise<string> {
  const abs = await join(projectRoot, relativePath);
  return convertFileSrc(abs);
}
