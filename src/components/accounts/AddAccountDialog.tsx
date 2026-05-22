import { useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useCreateAccount } from "@/hooks/useAccounts";
import { useProviderList } from "@/hooks/useProviders";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  defaultProviderId?: string;
}

export function AddAccountDialog({
  open,
  onOpenChange,
  defaultProviderId,
}: Props) {
  const providers = useProviderList();
  const create = useCreateAccount();

  const [providerId, setProviderId] = useState(defaultProviderId ?? "");
  const [label, setLabel] = useState("");
  const [apiKey, setApiKey] = useState("");

  useEffect(() => {
    if (open) {
      setProviderId(defaultProviderId ?? "");
      setLabel("");
      setApiKey("");
      create.reset();
    }
  }, [open, defaultProviderId]); // eslint-disable-line react-hooks/exhaustive-deps

  const trimmedLabel = label.trim();
  const trimmedKey = apiKey.trim();
  const disabled =
    create.isPending || !providerId || !trimmedLabel || !trimmedKey;

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (disabled) return;
    try {
      const account = await create.mutateAsync({
        provider_id: providerId,
        label: trimmedLabel,
        api_key: trimmedKey,
      });
      toast.success(`已添加账号「${account.label}」`);
      onOpenChange(false);
    } catch (err) {
      toast.error(
        `添加失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[460px]">
        <DialogHeader>
          <DialogTitle>添加账号</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label>Provider *</Label>
            <Select
              value={providerId}
              onValueChange={setProviderId}
              disabled={!!defaultProviderId || create.isPending}
            >
              <SelectTrigger>
                <SelectValue placeholder="选择 Provider" />
              </SelectTrigger>
              <SelectContent>
                {providers.data?.map((p) => (
                  <SelectItem key={p.id} value={p.id}>
                    {p.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-2">
            <Label htmlFor="account-label">账号备注 *</Label>
            <Input
              id="account-label"
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder="如：个人主账号"
              maxLength={100}
              autoFocus
              disabled={create.isPending}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="account-key">API Key *</Label>
            <Input
              id="account-key"
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-..."
              autoComplete="off"
              disabled={create.isPending}
            />
            <p className="text-xs text-muted-foreground">
              密钥仅保存在系统 keyring，本机不留明文，云端不上传。
            </p>
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={create.isPending}
            >
              取消
            </Button>
            <Button type="submit" disabled={disabled}>
              {create.isPending ? "添加中..." : "添加"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
