import { useEffect, useState } from "react";
import { join } from "@tauri-apps/api/path";

export function useResolvedPath(
  base: string | null | undefined,
  relative: string | null | undefined,
): string | null {
  const [resolved, setResolved] = useState<string | null>(null);

  useEffect(() => {
    if (!base || !relative) {
      setResolved(null);
      return;
    }
    let cancelled = false;
    join(base, relative).then((abs) => {
      if (!cancelled) setResolved(abs);
    });
    return () => {
      cancelled = true;
    };
  }, [base, relative]);

  return resolved;
}
