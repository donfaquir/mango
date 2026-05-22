import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useDeleteAccount } from "@/hooks/useAccounts";

interface Props {
  accountId: string;
  accountLabel: string;
  providerName: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function DeleteAccountDialog({
  accountId,
  accountLabel,
  providerName,
  open,
  onOpenChange,
}: Props) {
  const del = useDeleteAccount();

  const handleConfirm = async () => {
    try {
      await del.mutateAsync(accountId);
      toast.success(`已删除 ${providerName} 账号「${accountLabel}」`);
      onOpenChange(false);
    } catch (err) {
      toast.error(
        `删除失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[420px]">
        <DialogHeader>
          <DialogTitle>删除账号</DialogTitle>
          <DialogDescription>
            将永久移除 {providerName} 账号「{accountLabel}」，并清理 keyring
            中保存的密钥。此操作不可撤销。
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={del.isPending}
          >
            取消
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={handleConfirm}
            disabled={del.isPending}
          >
            {del.isPending ? "删除中..." : "确认删除"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
