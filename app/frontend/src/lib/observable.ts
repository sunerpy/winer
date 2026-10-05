import { useSyncExternalStore } from "react";

/** A value with subscribers, read in React through `useSyncExternalStore`. */
export class Observable<T> {
  #value: T;
  readonly #listeners = new Set<() => void>();

  constructor(value: T) {
    this.#value = value;
  }

  get = (): T => this.#value;

  set(value: T): void {
    if (Object.is(value, this.#value)) return;
    this.#value = value;
    for (const listener of this.#listeners) listener();
  }

  update(change: (value: T) => T): void {
    this.set(change(this.#value));
  }

  subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };
}

/** `select` must return a part of the value or a primitive, never a fresh object: React compares
 *  the result by identity and would re-render forever. */
export function useObservable<T, S = T>(observable: Observable<T>, select?: (value: T) => S): S {
  const read = () => (select ? select(observable.get()) : (observable.get() as unknown as S));
  return useSyncExternalStore(observable.subscribe, read, read);
}
