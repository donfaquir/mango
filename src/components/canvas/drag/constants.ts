// Custom MIME used by AssetDrawer drag sources so canvas drop handlers can
// tell internal asset drags apart from system file drops (which surface as
// `Files`). Keep in sync with the dataTransfer.setData / getData calls.
export const DRAG_MIME_ASSET_ID = "application/x-mango-asset-id";

// File extensions accepted by the canvas system-drop path. Anything else is
// toast-rejected before `import_asset` is invoked, so the import pipeline does
// not have to know about UI-level filtering.
export const ACCEPTED_EXTENSIONS = [
  "png",
  "jpg",
  "jpeg",
  "webp",
  "mp4",
  "mov",
  "webm",
] as const;

export type AcceptedExtension = (typeof ACCEPTED_EXTENSIONS)[number];

export function isAcceptedExtension(path: string): boolean {
  const ext = path.split(".").pop()?.toLowerCase();
  if (ext == null) return false;
  return (ACCEPTED_EXTENSIONS as readonly string[]).includes(ext);
}
