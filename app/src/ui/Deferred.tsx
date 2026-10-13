import { Suspense, lazy, useState, type ComponentType } from "react";
import { useApp } from "../store/app";

type State = ReturnType<typeof useApp.getState>;

/** A screen that is closed at launch, loaded the first time it's needed and kept mounted after,
 *  so its open and close animations behave as before. Its code isn't parsed or held in memory
 *  until then (part of the idle-memory budget). */
export function deferred(load: () => Promise<ComponentType>, needed: (s: State) => boolean): ComponentType {
  const Screen = lazy(() => load().then((C) => ({ default: C })));
  return function Deferred() {
    const now = useApp(needed);
    const [used, setUsed] = useState(false);
    if (now && !used) setUsed(true);
    if (!now && !used) return null;
    return (
      <Suspense fallback={null}>
        <Screen />
      </Suspense>
    );
  };
}
