interface DropTargetOverlayProps {
  show: boolean;
}

export function DropTargetOverlay({ show }: DropTargetOverlayProps) {
  if (!show) return null;
  return (
    <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center border-4 border-dashed border-blue-400 bg-blue-50/20">
      <p className="rounded-md bg-background/90 px-4 py-2 text-sm font-medium shadow">
        松开以添加到画布
      </p>
    </div>
  );
}
