import type { IpcError } from "@/lib/bindings/commands";

export class IpcCallError extends Error {
  readonly code: string;

  constructor(err: IpcError) {
    super(err.message);
    this.name = "IpcCallError";
    this.code = err.code;
  }
}

type IpcResult<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: IpcError };

/** Unwrap a tauri-specta Result. Throws `IpcCallError` on the error branch
 *  so TanStack Query / try-catch flows can handle it uniformly. */
export async function unwrap<T>(p: Promise<IpcResult<T>>): Promise<T> {
  const r = await p;
  if (r.status === "ok") return r.data;
  throw new IpcCallError(r.error);
}
