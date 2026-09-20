"use client";
import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useFinstack } from "../use-finstack/use-finstack";
import type { ValidationRequest } from "../../workers/finstack-contract";
/** Debounce 250ms; never attach data/errors from a different form revision or text. */
export function useValidateInstrument(
  request: ValidationRequest,
  enabled = true,
) {
  const worker = useFinstack();
  const { instrumentJson, revision } = request;
  const [debounced, setDebounced] = useState<ValidationRequest | null>(null);
  useEffect(() => {
    if (!enabled) return;
    const timer = setTimeout(
      () => setDebounced({ instrumentJson, revision }),
      250,
    );
    return () => clearTimeout(timer);
  }, [instrumentJson, revision, enabled]);
  const current =
    enabled &&
    debounced?.instrumentJson === instrumentJson &&
    debounced.revision === revision;
  const query = useQuery({
    queryKey: ["finstack", worker.session, "validate", debounced],
    queryFn: () => worker.client!.call("validate", debounced!),
    enabled: worker.status === "ready" && current,
    staleTime: Infinity,
    retry: false,
  });
  return {
    data: current ? query.data : undefined,
    error: worker.error ?? (current ? query.error : null),
    isValidating: enabled && (!current || query.isFetching),
    workerStatus: worker.status,
  };
}
