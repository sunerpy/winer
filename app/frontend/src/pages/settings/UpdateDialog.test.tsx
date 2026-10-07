import { describe, expect, it } from "vitest";

import { plainNotes } from "./UpdateDialog";

describe("update release notes", () => {
  it("drops a leading heading that repeats the version and keeps safe text", () => {
    expect(
      plainNotes(
        "## v0.0.8\n- **应用内更新**\n- [完整说明](https://example.invalid/release)",
        "0.0.8",
      ),
    ).toEqual(["- 应用内更新", "- 完整说明"]);
  });
});
