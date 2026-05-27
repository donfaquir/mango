import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { AlertTriangleIcon } from "lucide-react";

interface ErrorAlertProps {
  error: unknown;
  onRetry?: () => void;
  title?: string;
}

export function ErrorAlert({
  error,
  onRetry,
  title = "加载失败",
}: ErrorAlertProps) {
  const message =
    error instanceof Error ? error.message : String(error ?? "未知错误");
  return (
    <Alert variant="destructive">
      <AlertTriangleIcon className="size-4" />
      <AlertTitle>{title}</AlertTitle>
      <AlertDescription className="flex flex-col gap-2">
        <span>{message}</span>
        {onRetry && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={onRetry}
            className="w-fit"
          >
            重试
          </Button>
        )}
      </AlertDescription>
    </Alert>
  );
}
