"use client";
import { useEffect, useState } from "react";

/** Captures one completed valuation for printing and clears it on afterprint. */
export function usePrintSnapshot<T>(completed: T | null) {
  const [printSnapshot, setPrintSnapshot] = useState<T | null>(null);
  useEffect(() => {
    const finish = () => setPrintSnapshot(null);
    window.addEventListener("afterprint", finish);
    return () => window.removeEventListener("afterprint", finish);
  }, []);
  return {
    printSnapshot,
    printing: printSnapshot !== null,
    startPrint() {
      if (completed) setPrintSnapshot(completed);
    },
  };
}

/** Prints the captured snapshot once its dependent queries have settled and fonts are loaded. */
export function usePrintWhenReady(snapshot: unknown, preparing: boolean) {
  useEffect(() => {
    if (snapshot === null || preparing) return;
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
  }, [snapshot, preparing]);
}
