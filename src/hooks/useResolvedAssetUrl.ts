import { useEffect, useState } from "react";
import { resolveAssetUrl } from "@/lib/assetUrl";

/**
 * Resolve a project-relative asset path (e.g. `assets/<id>.png`) to a webview
 * URL via the Tauri asset protocol. Returns `null` while resolving or when
 * either input is missing.
 *
 * Plain useEffect + useState — `convertFileSrc` is synchronous, `join` is a
 * cheap path-format call. TanStack Query would be over-engineering.
 */
export function useResolvedAssetUrl(
  projectRoot: string | undefined | null,
  relativePath: string | null | undefined,
): string | null {
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    if (!projectRoot || !relativePath) {
      setUrl(null);
      return;
    }
    let cancelled = false;
    resolveAssetUrl(projectRoot, relativePath).then((resolved) => {
      if (!cancelled) setUrl(resolved);
    });
    return () => {
      cancelled = true;
    };
  }, [projectRoot, relativePath]);

  return url;
}
