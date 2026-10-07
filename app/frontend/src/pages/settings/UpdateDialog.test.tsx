import { describe, expect, it } from "vitest";

import { plainNotes } from "./UpdateDialog";

describe("update release notes", () => {
  it("drops a leading heading that repeats the version and keeps safe text", () => {
    expect(
      plainNotes(
        "## [0.0.8](https://example.invalid/release) (2026-10-07)\n- **应用内更新**\n- [完整说明](https://example.invalid/release)",
        "0.0.8",
      ),
    ).toEqual(["- 应用内更新", "- 完整说明"]);
  });
});
