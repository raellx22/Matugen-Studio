import { useEffect, useState, type Dispatch, type SetStateAction } from 'react';

/**
 * In-memory, module-scoped store shared by every `useSessionState` call.
 * Deliberately NOT backed by `localStorage`/`sessionStorage`: it must survive
 * a component unmount (e.g. navigating to another sidebar tab and back, or
 * minimizing the app to the tray) but reset naturally when the app process
 * itself restarts, since the store simply doesn't exist anymore at that point.
 */
const store = new Map<string, unknown>();

/**
 * Drop-in replacement for `useState` whose value is preserved across
 * unmount/remount cycles for the lifetime of the app process, keyed by
 * `key`. Use it for state that should feel "still there" when the user
 * navigates away and back within the same running session, without ever
 * persisting to disk.
 */
export function useSessionState<T>(
  key: string,
  initialValue: T | (() => T),
): [T, Dispatch<SetStateAction<T>>] {
  const [value, setValue] = useState<T>(() => {
    if (store.has(key)) {
      return store.get(key) as T;
    }
    const initial = initialValue instanceof Function ? initialValue() : initialValue;
    store.set(key, initial);
    return initial;
  });

  useEffect(() => {
    store.set(key, value);
    // `key` is expected to be a stable literal per call site, not itself state.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value]);

  return [value, setValue];
}
