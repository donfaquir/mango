import { Settings as SettingsIcon } from "lucide-react";
import { EmptyState } from "@/components/common/EmptyState";

export function GeneralSettingsPanel() {
  return (
    <EmptyState
      icon={SettingsIcon}
      title="通用设置"
      description="主题、语言、默认路径等设置项将在后续迭代中加入。"
    />
  );
}
