import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, it, expect, vi } from "vitest";

vi.mock("@/lib/bindings/commands", () => ({
  // App.tsx waits on `commands.getWorkspaceStatus()` to decide between
  // onboarding and the main router. Returning a null workspace_root drives
  // it down the onboarding branch, which renders without React Router and
  // exercises the boot gate without needing a memory router stub.
  commands: {
    getWorkspaceStatus: vi.fn(() =>
      Promise.resolve({ status: "ok", data: { workspace_root: null } }),
    ),
    probeWorkspace: vi.fn(() => Promise.resolve({ status: "ok", data: { kind: "empty" } })),
    pickProjectDirectory: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
    setWorkspaceAndRelaunch: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
  },
  events: {
    taskStatusChanged: { listen: vi.fn(() => Promise.resolve(() => {})) },
    taskEventLogged: { listen: vi.fn(() => Promise.resolve(() => {})) },
    taskProgressTick: { listen: vi.fn(() => Promise.resolve(() => {})) },
  },
}));

import App from "../App";

function renderWithProviders(ui: React.ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
  );
}

describe("App", () => {
  it("renders onboarding when no workspace is mounted", async () => {
    renderWithProviders(<App />);
    await waitFor(() => {
      expect(screen.getByText("欢迎使用 Mango")).toBeInTheDocument();
    });
  });
});
