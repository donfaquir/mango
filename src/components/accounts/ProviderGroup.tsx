import { useState } from "react";
import { ExternalLink, Plus, KeyRound } from "lucide-react";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/common/EmptyState";
import type { ApiAccount, Provider } from "@/lib/bindings/commands";
import { AccountRow } from "./AccountRow";
import { AddAccountDialog } from "./AddAccountDialog";

interface Props {
  provider: Provider;
  accounts: ApiAccount[];
}

export function ProviderGroup({ provider, accounts }: Props) {
  const [addOpen, setAddOpen] = useState(false);

  return (
    <section className="space-y-3">
      <header className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h3 className="text-base font-semibold">{provider.name}</h3>
          {provider.docs_url && (
            <a
              href={provider.docs_url}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
            >
              文档
              <ExternalLink className="h-3 w-3" />
            </a>
          )}
        </div>
        {accounts.length > 0 && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => setAddOpen(true)}
          >
            <Plus className="mr-1 h-4 w-4" />
            添加账号
          </Button>
        )}
      </header>

      {accounts.length === 0 ? (
        <EmptyState
          icon={KeyRound}
          title={`尚未配置 ${provider.name} 账号`}
          description="添加账号后即可使用对应模型生成内容。"
          action={
            <Button onClick={() => setAddOpen(true)}>
              <Plus className="mr-1 h-4 w-4" />
              添加账号
            </Button>
          }
        />
      ) : (
        <div className="space-y-2">
          {accounts.map((account) => (
            <AccountRow
              key={account.id}
              account={account}
              providerName={provider.name}
            />
          ))}
        </div>
      )}

      <AddAccountDialog
        open={addOpen}
        onOpenChange={setAddOpen}
        defaultProviderId={provider.id}
      />
    </section>
  );
}
