import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    getProject: vi.fn(() =>
      Promise.resolve({
        status: "ok",
        data: { id: "p1", root_path: "/tmp/project" },
      }),
    ),
  },
}));

vi.mock("@/hooks/useResolvedAssetUrl", () => ({
  useResolvedAssetUrl: () => null,
}));

import { SubjectCard } from "./SubjectCard";

function renderCard(subject: {
  id: string;
  name: string;
  description: string;
  reference_image_path: string | null;
}) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <SubjectCard
          subject={subject as never}
          kind="character"
          projectId="p1"
        />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("SubjectCard", () => {
  it("renders name and description", () => {
    renderCard({
      id: "c1",
      name: "主角小明",
      description: "高中生，性格活泼",
      reference_image_path: null,
    });
    expect(screen.getByText("主角小明")).toBeInTheDocument();
    expect(screen.getByText("高中生，性格活泼")).toBeInTheDocument();
  });

  it("links to the subject detail path", () => {
    renderCard({
      id: "c1",
      name: "角色 A",
      description: "",
      reference_image_path: null,
    });
    const link = screen.getByRole("link");
    expect(link).toHaveAttribute(
      "href",
      "/project/p1/subjects/character/c1",
    );
  });
});
