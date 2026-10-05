import { type DependencyList, useCallback, useEffect, useRef, useState } from "react";

export interface AsyncState<T> {
  data: T | undefined;
  error: unknown;
  loading: boolean;
  reload: () => void;
}

/** Runs `load` whenever `deps` change; a late answer to an earlier request is dropped. With
 *  `enabled` false nothing runs and the state is cleared. */
export function useAsync<T>(
  load: () => Promise<T>,
  deps: DependencyList,
  enabled = true,
): AsyncState<T> {
  const [state, setState] = useState<{ data?: T; error?: unknown; loading: boolean }>({
    loading: enabled,
  });
  const [nonce, setNonce] = useState(0);
  const request = useRef(0);
  const latest = useRef(load);
  // Declared before the loading effect, so it has run by the time that one reads it.
  useEffect(() => {
    latest.current = load;
  });

  useEffect(() => {
    const id = ++request.current;
    if (!enabled) {
      setState({ loading: false });
      return;
    }
    setState((previous) => ({ data: previous.data, loading: true }));
    latest
      .current()
      .then((data) => {
        if (request.current === id) setState({ data, loading: false });
      })
      .catch((error: unknown) => {
        if (request.current === id) setState({ error, loading: false });
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the caller's deps decide when to load
  }, [...deps, enabled, nonce]);

  const reload = useCallback(() => setNonce((value) => value + 1), []);
  return { data: state.data, error: state.error, loading: state.loading, reload };
}
