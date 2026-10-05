import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useRef, useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { Dialog } from "./Dialog";
import { CommitInput } from "./Input";
import { pageItems } from "./Pager";
import { Popover, placePanel } from "./Popover";
import { Segmented } from "./Segmented";
import { Toggle } from "./Toggle";

function DialogHarness({ dismissOnScrim = false }: { dismissOnScrim?: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        open
      </button>
      <Dialog
        open={open}
        onClose={() => setOpen(false)}
        title="Title"
        closeLabel="Close"
        dismissOnScrim={dismissOnScrim}
        footer={<button type="button">last</button>}
      >
        <input aria-label="field" />
        <button type="button" disabled>
          disabled
        </button>
        <div hidden>
          <button type="button">hidden</button>
        </div>
      </Dialog>
    </>
  );
}

describe("Dialog", () => {
  it("takes focus, wraps Tab at both edges and gives focus back on Escape", async () => {
    const user = userEvent.setup();
    render(<DialogHarness />);
    await user.click(screen.getByRole("button", { name: "open" }));

    const close = screen.getByRole("button", { name: "Close" });
    expect(close).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("textbox", { name: "field" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "last" })).toHaveFocus();
    await user.tab();
    expect(close).toHaveFocus();
    await user.tab({ shift: true });
    expect(screen.getByRole("button", { name: "last" })).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("button", { name: "open" })).toHaveFocus();
  });

  it("closes on the scrim only where the call site allows it", async () => {
    const user = userEvent.setup();
    const { unmount } = render(<DialogHarness />);
    await user.click(screen.getByRole("button", { name: "open" }));
    fireEvent.click(screen.getByTestId("dialog-scrim"));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    unmount();

    render(<DialogHarness dismissOnScrim />);
    await user.click(screen.getByRole("button", { name: "open" }));
    fireEvent.click(screen.getByTestId("dialog-scrim"));
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

function SegmentedHarness() {
  const [value, setValue] = useState<"a" | "b" | "c">("a");
  return (
    <Segmented
      label="pick"
      value={value}
      onChange={setValue}
      options={[
        { value: "a", label: "A" },
        { value: "b", label: "B", disabled: true },
        { value: "c", label: "C" },
      ]}
    />
  );
}

describe("Segmented", () => {
  it("is one Tab stop whose arrows skip disabled options and wrap", async () => {
    const user = userEvent.setup();
    render(<SegmentedHarness />);
    const [a, , c] = screen.getAllByRole("radio");
    expect(a).toHaveAttribute("tabindex", "0");
    expect(c).toHaveAttribute("tabindex", "-1");

    await user.tab();
    expect(a).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(c).toBeChecked();
    expect(c).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(a).toBeChecked();
    await user.keyboard("{ArrowLeft}");
    expect(c).toBeChecked();
  });
});

describe("pageItems", () => {
  it("keeps the first, the last and two either side, and ends in a gap while more may follow", () => {
    expect(pageItems(1, 0, true)).toEqual([1, "gap"]);
    expect(pageItems(1, 3, false)).toEqual([1, 2, 3]);
    expect(pageItems(2, 5, true)).toEqual([1, 2, 3, 4, 5, "gap"]);
    expect(pageItems(6, 16, false)).toEqual([1, "gap", 4, 5, 6, 7, 8, "gap", 16]);
    expect(pageItems(7, 6, true), "a page asked for before it arrives").toEqual([
      1,
      "gap",
      5,
      6,
      7,
      "gap",
    ]);
  });
});

describe("Toggle", () => {
  it("is a named switch that reports its state", async () => {
    const user = userEvent.setup();
    function Harness() {
      const [checked, setChecked] = useState(false);
      return <Toggle checked={checked} onChange={setChecked} label="Auto accept" />;
    }
    render(<Harness />);
    const toggle = screen.getByRole("switch", { name: "Auto accept" });
    expect(toggle).not.toBeChecked();
    await user.click(toggle);
    expect(toggle).toBeChecked();
  });
});

describe("CommitInput", () => {
  it("commits once, when the field is left or Enter is pressed", async () => {
    const user = userEvent.setup();
    const commit = vi.fn();
    render(<CommitInput aria-label="name" value="上等马" onCommit={commit} />);
    const field = screen.getByRole("textbox", { name: "name" });
    await user.clear(field);
    await user.type(field, "大腿");
    expect(commit).not.toHaveBeenCalled();
    await user.keyboard("{Enter}");
    expect(commit).toHaveBeenCalledExactlyOnceWith("大腿");
    await user.click(field);
    await user.tab();
    expect(commit).toHaveBeenCalledOnce();
  });
});

describe("Popover", () => {
  const view = { width: 1240, height: 800 };
  const button = { left: 1087, top: 392, right: 1176, bottom: 420 };

  it("sits on the preferred side and stays inside the window", () => {
    expect(placePanel(button, 340, 258, "bottom-end", view)).toEqual({ left: 836, top: 426 });
    // Left-aligned it would run past the right edge: it is pushed back in instead.
    expect(placePanel(button, 340, 258, "bottom-start", view)).toEqual({ left: 892, top: 426 });
  });

  it("flips to the other side when its own has no room", () => {
    const low = { left: 1087, top: 744, right: 1176, bottom: 772 };
    expect(placePanel(low, 340, 190, "bottom-end", view).top).toBe(744 - 6 - 190);
    const high = { left: 20, top: 30, right: 200, bottom: 60 };
    expect(placePanel(high, 176, 200, "top-start", view).top).toBe(66);
    expect(placePanel(button, 2000, 2000, "bottom-start", view)).toEqual({ left: 8, top: 8 });
  });

  it("draws outside the page so it cannot widen a scroller, and gives focus to its field", async () => {
    const user = userEvent.setup();
    function Harness() {
      const [open, setOpen] = useState(false);
      const anchor = useRef<HTMLDivElement>(null);
      return (
        <main data-testid="scroller" style={{ overflow: "auto" }}>
          <div ref={anchor}>
            <button type="button" onClick={() => setOpen(true)}>
              add
            </button>
          </div>
          <Popover open={open} onClose={() => setOpen(false)} anchor={anchor} label="picker">
            <input aria-label="search" autoFocus />
          </Popover>
        </main>
      );
    }
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "add" }));
    const panel = screen.getByRole("dialog", { name: "picker" });
    expect(panel.parentElement).toBe(document.body);
    expect(screen.getByTestId("scroller")).not.toContainElement(panel);
    expect(screen.getByRole("textbox", { name: "search" })).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "picker" })).toBeNull();
  });
});
