"use client";
import type { ReactNode } from "react";
import { useIsFetching } from "@tanstack/react-query";
import {
  FinstackQueryProvider,
  useFinstack,
} from "@/hooks/shared/use-finstack/use-finstack";
function Ready({ children }: { children: ReactNode }) {
  const worker = useFinstack(),
    fetching = useIsFetching();
  return (
    <div data-worker-state={worker.status} data-fetching={fetching}>
      {worker.error && <p role="alert">{worker.error.message}</p>}
      {children}
    </div>
  );
}
/** Mount the Rust worker around a preview and expose its readiness to the e2e probe. */
export function WorkerPreview({ children }: { children: ReactNode }) {
  return (
    <FinstackQueryProvider>
      <Ready>{children}</Ready>
    </FinstackQueryProvider>
  );
}
