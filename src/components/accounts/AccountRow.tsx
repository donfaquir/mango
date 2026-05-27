import { useState } from "react";
import { CheckCircle2, Cloud, KeyRound, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import type { ApiAccount } from "@/lib/bindings/commands";
import { useVerifyApiAccountStorage } from "@/hooks/useAccounts";
import { DeleteAccountDialog } from "./DeleteAccountDialog";

interface Props {
  account: ApiAccount;
  providerName: string;
}

export function AccountRow({ account, providerName }: Props) {
  const verify = useVerifyApiAccountStorage();
  const [deleteOpen, setDeleteOpen] = useState(false);

  const handleVerify = async () => {
    try {
      await verify.mutateAsync(account.id);
      toast.success("密钥存储正常", {
        icon: <CheckCircle2 className="h-4 w-4 text-green-500" />,
      });
    } catch (err) {
      toast.error(
        `密钥校验失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  return (
    <>
      <div className="flex items-center justify-between rounded-md border bg-card px-4 py-3">
        <div className="flex min-w-0 flex-1 items-center gap-3">
          <KeyRound className="h-4 w-4 shrink-0 text-muted-foreground" />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <p className="truncate font-medium">{account.label}</p>
              {account.oss_config && (
                <span
                  className="inline-flex shrink-0 items-center gap-1 rounded-full border border-sky-200 bg-sky-50 px-1.5 py-0.5 text-[10px] font-medium text-sky-700 dark:border-sky-900 dark:bg-sky-950/40 dark:text-sky-300"
                  title={`OSS: ${account.oss_config.bucket} @ ${account.oss_config.endpoint}`}
                >
                  <Cloud className="h-3 w-3" />
                  OSS
                </span>
              )}
            </div>
            <p className="text-xs text-muted-foreground">
              ****{account.key_last4}
              {account.last_used_at &&
                ` · 上次使用 ${new Date(account.last_used_at).toLocaleDateString()}`}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={handleVerify}
            disabled={verify.isPending}
          >
            {verify.isPending ? "校验中..." : "校验密钥存储"}
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => setDeleteOpen(true)}
            aria-label={`删除 ${account.label}`}
          >
            <Trash2 className="h-4 w-4" />
          </Button>
        </div>
      </div>
      <DeleteAccountDialog
        accountId={account.id}
        accountLabel={account.label}
        providerName={providerName}
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
      />
    </>
  );
}
