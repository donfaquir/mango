import { describe, expect, it } from "vitest";
import { parseDbDate } from "./datetime";

describe("parseDbDate", () => {
  it("treats SQLite 'YYYY-MM-DD HH:MM:SS' as UTC, not local", () => {
    const d = parseDbDate("2026-05-29 10:30:00");
    expect(d.toISOString()).toBe("2026-05-29T10:30:00.000Z");
  });

  it("handles ISO 8601 with T separator and no timezone", () => {
    const d = parseDbDate("2026-05-29T10:30:00");
    expect(d.toISOString()).toBe("2026-05-29T10:30:00.000Z");
  });

  it("preserves explicit Z suffix", () => {
    const d = parseDbDate("2026-05-29T10:30:00Z");
    expect(d.toISOString()).toBe("2026-05-29T10:30:00.000Z");
  });

  it("preserves explicit numeric offset", () => {
    const d = parseDbDate("2026-05-29T10:30:00+08:00");
    expect(d.toISOString()).toBe("2026-05-29T02:30:00.000Z");
  });

  it("handles fractional seconds", () => {
    const d = parseDbDate("2026-05-29 10:30:00.123");
    expect(d.toISOString()).toBe("2026-05-29T10:30:00.123Z");
  });
});
