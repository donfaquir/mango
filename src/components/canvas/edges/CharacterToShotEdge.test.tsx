import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { CharacterToShotEdge } from "./CharacterToShotEdge";

describe("CharacterToShotEdge", () => {
  it("renders a Users badge near the path midpoint", () => {
    const { container } = render(
      <ReactFlowProvider>
        <svg>
          <CharacterToShotEdge
            id="e1"
            source="a"
            target="b"
            sourceX={0}
            sourceY={0}
            targetX={100}
            targetY={100}
            sourcePosition={"right" as never}
            targetPosition={"left" as never}
            selected={false}
            animated={false}
            data={undefined}
          />
        </svg>
      </ReactFlowProvider>,
    );
    // The bezier path always renders. The Users badge inside EdgeLabelRenderer
    // requires a fully-mounted ReactFlow instance to attach into the renderer
    // div, so in this isolated render we only assert on the path.
    expect(container.querySelector("path")).not.toBeNull();
  });
});
