import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, it, expect, vi } from "vitest";

vi.mock("@/lib/bindings/commands", () => ({
  events: {
    taskStatusChanged: {
      listen: vi.fn(() => Promise.resolve(() => {})),
    },
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
  it("renders the app title", () => {
    renderWithProviders(<App />);
    expect(screen.getByText("Mango")).toBeInTheDocument();
  });
});
