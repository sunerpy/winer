// 隐藏首页推广 on the client's home page: the pop-ups hidden, Tencent's hub replaced by winer's note
// and never the Home tab itself.
import type { Settings, Snapshot } from "@winer/shared";
import { afterEach, describe, expect, it } from "vitest";

import { PROMOTIONS, decorateHome, hubShown } from "./home";
import { Controller } from "./index";

/** The Home tab as the Tencent client (16.19) draws it, the hub's pages left out. */
function activityCentre(): { root: HTMLElement; section: HTMLElement; hub: HTMLIFrameElement } {
  const root = document.createElement("div");
  root.className = "screen-root active";
  root.dataset.screenName = "rcp-fe-lol-activity-center";
  root.innerHTML = `<div class="activity-center-application"><section id="activity-center" class="activity-center-ready"><main class="activity-center__contents"><lol-uikit-section-controller animation><div class="managed-iframe ember-view"><div class="managed-iframe-wrapper"><iframe frameborder="0"></iframe></div></div></lol-uikit-section-controller><section class="activity-center__persistent-layer-container"></section></main><div class="persistent-control-panel ember-view"></div></section></div>`;
  document.body.append(root);
  return {
    root,
    section: root.querySelector("section#activity-center") as HTMLElement,
    hub: root.querySelector("iframe") as HTMLIFrameElement,
  };
}

/** The esports pop-up, waiting at the client's own inline `display: none`. */
function contestPop(): HTMLIFrameElement {
  const pop = document.createElement("iframe");
  pop.id = "contestPop";
  pop.setAttribute(
    "style",
    "width: 50%; height: 50%; border: none; display: none; top: 0px; left: 0px;",
  );
  document.body.append(pop);
  return pop;
}

function settings(hidePromotions: boolean, language = "zh-CN"): Settings {
  return { general: { language }, plugin: { hidePromotions } } as unknown as Settings;
}

const snapshot: Snapshot = {
  rev: 1,
  connection: { status: "searching" },
  me: null,
  phase: "None",
  champSelect: null,
  game: null,
  friends: null,
  lobby: null,
};

/** A controller that knows its settings and draws. */
function drawing(hide: boolean): Controller {
  const controller = new Controller(document);
  controller.state.hello(snapshot, settings(hide));
  controller.render();
  return controller;
}

const hidden = (element: Element) => getComputedStyle(element).display === "none";

afterEach(() => {
  document.body.replaceChildren();
  document.getElementById("winer-tweaks")?.remove();
  for (const name of ["data-winer-owner", "data-winer-beat", "data-winer-hub"])
    document.documentElement.removeAttribute(name);
  sessionStorage.clear();
});

describe("the option's stylesheet", () => {
  it("hides the pop-ups and the hub beside winer's note, never the Home tab", () => {
    const { root, section, hub } = activityCentre();
    const pop = contestPop();
    const old = document.createElement("iframe");
    old.id = "tv-official-pop";
    document.body.append(old);
    document.head.append(
      Object.assign(document.createElement("style"), {
        id: "winer-tweaks",
        textContent: PROMOTIONS,
      }),
    );

    for (const element of [root, section, section.querySelector("main") as Element])
      expect(hidden(element), `${element.tagName} stays as the client draws it`).toBe(false);
    expect(hidden(hub), "no note, no hiding: the hub stays").toBe(false);
    pop.style.display = "block";
    expect([hidden(pop), hidden(old)], "a pop-up stays hidden when it pops").toEqual([true, true]);

    const note = document.createElement("div");
    note.className = "winer-home";
    hub.after(note);
    expect(hidden(hub)).toBe(true);
    // The client may put its iframe back on either side of the note.
    hub.before(note);
    expect(hidden(hub)).toBe(true);
    note.remove();
    expect(hidden(hub)).toBe(false);
  });
});

describe("the home page", () => {
  it("puts winer's note in the hub's own box while the option is on", () => {
    const { section, hub } = activityCentre();
    drawing(true);
    const note = document.querySelector<HTMLElement>(".winer-home");
    expect(note?.parentElement, "beside the hub, in its box").toBe(hub.parentElement);
    expect(hub.parentElement?.firstElementChild, "the box still starts with the hub").toBe(hub);
    expect(note?.textContent).toContain("首页推广已按 winer 的“隐藏首页推广”选项隐藏。");
    expect(note?.querySelector("button")?.textContent).toBe("暂时显示");
    expect(hidden(hub)).toBe(true);
    expect(hidden(section)).toBe(false);
  });

  it("takes the note and the rules off when the option is switched off", () => {
    const { hub } = activityCentre();
    const pop = contestPop();
    pop.style.display = "block";
    const controller = drawing(true);
    expect(hidden(pop)).toBe(true);

    controller.state.event({ type: "settings", data: settings(false) });
    controller.render();
    expect(document.querySelector(".winer-home")).toBeNull();
    expect(document.getElementById("winer-tweaks")).toBeNull();
    expect([hidden(hub), hidden(pop)]).toEqual([false, false]);
  });

  it("shows the hub again for the session from the note's button", () => {
    const { hub } = activityCentre();
    let client = 0;
    hub.parentElement?.addEventListener("click", () => (client += 1));
    const controller = drawing(true);

    document.querySelector<HTMLElement>(".winer-home-show")?.click();
    expect(client, "the click is not the client's").toBe(0);
    expect(document.querySelector(".winer-home")).toBeNull();
    expect(hidden(hub)).toBe(false);
    controller.render();
    expect(document.querySelector(".winer-home"), "and the note stays away").toBeNull();
    expect(hidden(hub)).toBe(false);

    // A reload of the page keeps it, through the page's session storage.
    document.documentElement.removeAttribute("data-winer-hub");
    expect(hubShown(document)).toBe(true);
    // So does the time before the bridge's hello, when the settings are unknown.
    const fresh = new Controller(document);
    document.documentElement.removeAttribute("data-winer-owner");
    fresh.render();
    expect(hubShown(document)).toBe(true);
  });

  it("hides the hub again once the option is switched off and on", () => {
    activityCentre();
    const controller = drawing(true);
    document.querySelector<HTMLElement>(".winer-home-show")?.click();
    controller.state.event({ type: "settings", data: settings(false) });
    controller.render();
    controller.state.event({ type: "settings", data: settings(true) });
    controller.render();
    expect(document.querySelector(".winer-home")).not.toBeNull();
  });

  it("keeps its note as it is, and follows the language", () => {
    activityCentre();
    const controller = drawing(true);
    const note = document.querySelector(".winer-home");
    controller.render();
    expect(document.querySelector(".winer-home"), "an unchanged note is kept").toBe(note);

    controller.state.event({ type: "settings", data: settings(true, "en") });
    controller.render();
    expect(document.querySelectorAll(".winer-home")).toHaveLength(1);
    expect(document.querySelector(".winer-home-show")?.textContent).toBe("Show for now");
  });

  it("does nothing where the activity centre or its hub is missing", () => {
    document.body.innerHTML = `<main class="activity-center__contents"><iframe></iframe></main><section id="activity-center"><main class="activity-center__contents"></main></section>`;
    const before = document.body.innerHTML;
    expect(decorateHome(document, true, "zh-CN", "a")).toBe("missing");
    drawing(true);
    expect(document.body.innerHTML).toBe(before);

    document.body.replaceChildren();
    expect(decorateHome(document, true, "zh-CN", "a"), "an empty page").toBe("missing");
    expect(document.body.children).toHaveLength(0);
  });

  it("draws one note from the context that draws, and takes over another's", () => {
    const { hub } = activityCentre();
    const first = drawing(true);
    const second = new Controller(document);
    second.state.hello(snapshot, settings(true));
    second.render();
    const notes = () => [...document.querySelectorAll<HTMLElement>(".winer-home")];
    expect(notes().map((note) => note.dataset.winerContext)).toEqual([first.context]);

    // The first context goes quiet; the second takes over and draws its own note in its place.
    document.documentElement.dataset.winerBeat = "0";
    second.render();
    expect(notes().map((note) => note.dataset.winerContext)).toEqual([second.context]);
    expect(notes()[0]?.parentElement).toBe(hub.parentElement);
    notes()[0]?.querySelector<HTMLElement>(".winer-home-show")?.click();
    expect(notes()).toHaveLength(0);
    expect(hidden(hub)).toBe(false);
  });
});
