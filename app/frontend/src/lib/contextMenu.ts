// No browser context menu (Reload, Inspect) in the app, except where text is edited: without it
// there is no paste.
export function shouldSuppressContextMenu(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return true;
  return !target.closest('input, textarea, [contenteditable]:not([contenteditable="false"])');
}

export function installContextMenuPolicy(doc: Document): () => void {
  const onContextMenu = (event: MouseEvent) => {
    if (shouldSuppressContextMenu(event.target)) event.preventDefault();
  };
  doc.addEventListener("contextmenu", onContextMenu);
  return () => doc.removeEventListener("contextmenu", onContextMenu);
}
