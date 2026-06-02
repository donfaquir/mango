import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import type { NodeProps } from "@xyflow/react";

let characterData: { id: string; name: string; reference_image_path: string | null } | undefined;
let resolved: string | null = null;

vi.mock("@/hooks/useCharacters", () => ({
  useCharacter: () => ({ data: characterData }),
}));
vi.mock("@/hooks/useResolvedAssetUrl", () => ({
  useResolvedAssetUrl: () => resolved,
}));
vi.mock("./shared/useCurrentProjectRoot", () => ({
  useCurrentProjectRoot: () => "/tmp/project",
}));

import { CharacterNode } from "./CharacterNode";
import type { CharacterNode as CharacterNodeType } from "./types";
import { NodeTestProviders } from "./test-utils";

function renderNode() {
  const props = {
    id: "character-c1",
    data: { kind: "character", characterId: "c1" },
    selected: false,
    type: "character",
    zIndex: 0,
    isConnectable: true,
    xPos: 0,
    yPos: 0,
    dragging: false,
  } as unknown as NodeProps<CharacterNodeType>;
  return render(
    <NodeTestProviders>
      <CharacterNode {...props} />
    </NodeTestProviders>,
  );
}

describe("CharacterNode", () => {
  beforeEach(() => {
    characterData = {
      id: "c1",
      name: "小明",
      reference_image_path: "assets/c1.png",
    };
    resolved = null;
  });

  it("renders the character name", () => {
    renderNode();
    expect(screen.getByText("小明")).toBeInTheDocument();
  });

  it("renders an <img> with the resolved url when the reference is present", () => {
    resolved = "asset://resolved/c1.png";
    renderNode();
    const img = screen.getByRole("img");
    expect(img.getAttribute("src")).toBe("asset://resolved/c1.png");
  });

  it("falls back to placeholder icon when no resolved url", () => {
    resolved = null;
    renderNode();
    expect(screen.queryByRole("img")).toBeNull();
  });
});
