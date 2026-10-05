// Pengu Loader's own notices. Pengu v1.1 opens a welcome dialog in the client until "Do not show
// again" is ticked, then says "Pengu Loader is active!" on every start, and announces a newer Pengu
// when one is out. To someone who installed winer and never Pengu they explain nothing: winer
// installed the loader and keeps it current. The dialog is told it was seen before Pengu draws it,
// on window load, and the toasts of Pengu's own toaster that name Pengu are hidden as they
// appear. Other plugins' toasts are left alone.

interface DataStore {
  get(key: string, fallback?: unknown): unknown;
  set(key: string, value: unknown): boolean;
}

/** The key Pengu's welcome dialog reads; `false` means it was dismissed for good. */
const WELCOME = "pengu-welcome";
/** The element Pengu draws its own interface in, behind an open shadow root. */
const ROOT = "pengu-root";
/** The rule Pengu's toaster puts in its own container's style element. */
const TOASTER = ".sldt-active";

export function quietPengu(
  doc: Document = document,
  store: DataStore | undefined = (globalThis as { DataStore?: DataStore }).DataStore,
): void {
  try {
    if (store && store.get(WELCOME, true) !== false) store.set(WELCOME, false);
  } catch {
    // A DataStore of another shape: the dialog then shows, as it would without winer.
  }
  const watch = (root: ShadowRoot) => {
    const sweep = () => void hidePenguToasts(root);
    sweep();
    new MutationObserver(sweep).observe(root, { childList: true, subtree: true });
  };
  const host = doc.getElementById(ROOT);
  if (host?.shadowRoot) {
    watch(host.shadowRoot);
    return;
  }
  const observer = new MutationObserver(() => {
    const found = doc.getElementById(ROOT);
    if (!found?.shadowRoot) return;
    observer.disconnect();
    watch(found.shadowRoot);
  });
  observer.observe(doc.documentElement, { childList: true, subtree: true });
}

/** Hides the toasts in Pengu's own toaster that name Pengu; returns how many it hid. */
export function hidePenguToasts(root: ParentNode): number {
  let hidden = 0;
  for (const style of root.querySelectorAll("style")) {
    const container = style.parentElement;
    if (!container || !style.textContent?.includes(TOASTER)) continue;
    for (const toast of container.children) {
      if (toast === style || !(toast instanceof HTMLElement)) continue;
      if (toast.style.display !== "none" && toast.textContent?.includes("Pengu")) {
        toast.style.display = "none";
        hidden++;
      }
    }
  }
  return hidden;
}
