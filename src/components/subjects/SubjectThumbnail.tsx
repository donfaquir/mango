import { ImageIcon } from "lucide-react";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";
import { useProject } from "@/hooks/useProjects";

interface SubjectThumbnailProps {
  projectId: string;
  relativePath: string | null;
}

export function SubjectThumbnail({
  projectId,
  relativePath,
}: SubjectThumbnailProps) {
  const project = useProject(projectId);
  const url = useResolvedAssetUrl(project.data?.root_path, relativePath);

  if (!url) {
    return (
      <div
        className="flex aspect-square w-full items-center justify-center rounded bg-muted"
        aria-label="无缩略图"
      >
        <ImageIcon className="size-8 text-muted-foreground/40" />
      </div>
    );
  }
  return (
    <img
      src={url}
      alt=""
      className="aspect-square w-full rounded object-cover"
    />
  );
}
