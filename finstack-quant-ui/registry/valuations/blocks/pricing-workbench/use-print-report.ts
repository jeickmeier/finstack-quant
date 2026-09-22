"use client";
import { useEffect, useState } from "react";

/** Captures one completed valuation, prints after its queries settle, and clears on afterprint. */
export function usePrintReport<T>(completed: T | null) {
  const [printSnapshot, setPrintSnapshot] = useState<T | null>(null);
  const [pending, setPending] = useState(false);
  useEffect(() => {
    const finish = () => setPrintSnapshot(null);
    window.addEventListener("afterprint", finish);
    return () => window.removeEventListener("afterprint", finish);
  }, []);
  useEffect(() => {
    if (!printSnapshot || pending) return;
    let active = true;
    void document.fonts.ready.then(() =>
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          if (active) window.print();
        }),
      ),
    );
    return () => {
      active = false;
    };
  }, [printSnapshot, pending]);
  return {
    printSnapshot,
    printing: printSnapshot !== null,
    startPrint() {
      if (completed) setPrintSnapshot(completed);
    },
    /** Query flags are known only after this hook, so the caller reports them during render. */
    syncPending(next: boolean) {
      if (next !== pending) setPending(next);
    },
  };
}
