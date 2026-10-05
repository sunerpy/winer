import { useEffect, useState } from "react";

import { applyAppearance, useSystemDark } from "./lib/appearance";
import type { Backend } from "./lib/backend";
import { useObservable } from "./lib/observable";
import { AppStore, StoreContext } from "./lib/store";
import { Shell } from "./shell/Shell";

function Appearance({ store }: { store: AppStore }) {
  const settings = useObservable(store.settings);
  const systemDark = useSystemDark();
  useEffect(() => {
    if (settings) applyAppearance(settings.appearance, systemDark);
  }, [settings, systemDark]);
  return null;
}

export function App({ backend }: { backend: Backend }) {
  const [store] = useState(() => new AppStore(backend));
  const settings = useObservable(store.settings);

  useEffect(() => {
    let stop: (() => void) | undefined;
    let disposed = false;
    store
      .start()
      .then((unsubscribe) => {
        if (disposed) unsubscribe();
        else stop = unsubscribe;
      })
      .catch((error: unknown) => console.error("could not reach the core", error));
    return () => {
      disposed = true;
      stop?.();
    };
  }, [store]);

  return (
    <StoreContext value={store}>
      <Appearance store={store} />
      {/* Nothing renders before the settings arrive: every page may rely on them. */}
      {settings && <Shell />}
    </StoreContext>
  );
}
