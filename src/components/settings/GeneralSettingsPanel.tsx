import { CliToolsSection } from "./CliToolsSection";
import { GenerationSettingsSection } from "./GenerationSettingsSection";
import { WorkspaceSettingsSection } from "./WorkspaceSettingsSection";

export function GeneralSettingsPanel() {
  return (
    <div className="space-y-4">
      <WorkspaceSettingsSection />
      <GenerationSettingsSection />
      <CliToolsSection />
    </div>
  );
}
