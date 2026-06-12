import { convertFileSrc } from "@tauri-apps/api/core";
import { Skeleton } from "@/components/ui/skeleton";
import { useAsset } from "@/hooks/useAssets";
import { useThumbnailStrip } from "@/hooks/useThumbnailStrip";
import { cn } from "@/lib/utils";

interface VideoClipThumbnailsProps {
  assetId: string;
  projectRoot: string | undefined;
  className?: string;
}

export function VideoClipThumbnails({
  assetId,
  projectRoot,
  className,
}: VideoClipThumbnailsProps) {
  const {
    data: asset,
    error: assetError,
    isLoading: assetLoading,
  } = useAsset(assetId);

  const videoPath =
    projectRoot && asset?.file_path
      ? `${projectRoot}/${asset.file_path}`
      : null;

  const { data, isLoading: thumbLoading, error: thumbError } = useThumbnailStrip(videoPath);

  if (assetError || thumbError) {
    console.error("[VideoClipThumbnails] failed", {
      assetId,
      projectRoot,
      videoPath,
      assetError,
      thumbError,
    });
    return null;
  }

  if (!projectRoot || (!asset && !assetLoading)) return null;

  if (assetLoading || thumbLoading || !data) {
    return <Skeleton className={cn("h-full w-full rounded-none", className)} />;
  }

  const count = data.thumbnails.length;
  if (count === 0) return null;

  return (
    <div className={cn("flex h-full w-full overflow-hidden", className)}>
      {data.thumbnails.map((path, i) => (
        <img
          key={i}
          src={convertFileSrc(path)}
          alt=""
          className="h-full object-cover flex-1 min-w-0"
          draggable={false}
        />
      ))}
    </div>
  );
}
