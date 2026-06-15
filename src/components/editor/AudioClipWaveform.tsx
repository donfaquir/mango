import { useState } from "react";
import { Music } from "lucide-react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Skeleton } from "@/components/ui/skeleton";
import { useAsset } from "@/hooks/useAssets";
import { useWaveform } from "@/hooks/useWaveform";
import { cn } from "@/lib/utils";

interface AudioClipWaveformProps {
  assetId: string;
  projectRoot: string | undefined;
  className?: string;
}

function FallbackIcon({ className }: { className?: string }) {
  return (
    <div
      className={cn(
        "flex items-center justify-center h-full w-full bg-muted/30",
        className,
      )}
    >
      <Music className="size-4 text-muted-foreground" />
    </div>
  );
}

export function AudioClipWaveform({
  assetId,
  projectRoot,
  className,
}: AudioClipWaveformProps) {
  const {
    data: asset,
    error: assetError,
    isLoading: assetLoading,
  } = useAsset(assetId);

  const audioPath =
    projectRoot && asset?.file_path
      ? `${projectRoot}/${asset.file_path}`
      : null;

  const {
    data,
    isLoading: waveformLoading,
    error: waveformError,
  } = useWaveform(audioPath);

  const [imgFailed, setImgFailed] = useState(false);

  if (assetError || waveformError) {
    return <FallbackIcon className={className} />;
  }

  if (!projectRoot || (!asset && !assetLoading)) {
    return <FallbackIcon className={className} />;
  }

  if (assetLoading || waveformLoading) {
    return <Skeleton className={cn("h-full w-full rounded-none", className)} />;
  }

  if (!data) {
    return <FallbackIcon className={className} />;
  }

  if (imgFailed) {
    return <FallbackIcon className={className} />;
  }

  return (
    <div className={cn("h-full w-full overflow-hidden", className)}>
      <img
        src={convertFileSrc(data.path)}
        alt=""
        className="h-full w-full object-fill"
        draggable={false}
        onError={() => setImgFailed(true)}
      />
    </div>
  );
}
