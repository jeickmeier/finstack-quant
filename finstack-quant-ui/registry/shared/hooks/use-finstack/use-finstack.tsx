"use client";
import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createClient, type FinstackClient } from "./client";
import { FinstackError, errorValue } from "@/workers/finstack-contract";
interface WorkerState {
  status: "starting" | "ready" | "error";
  client: FinstackClient | null;
  error: FinstackError | null;
  session: number;
  reset: () => void;
}
const Context = createContext<WorkerState | null>(null);
let nextSession = 0;
/** One browser worker per provider. Consumers share it; an existing QueryClientProvider is supported. */
export function FinstackProvider({
  children,
  wasmUrl,
}: {
  children: ReactNode;
  wasmUrl?: string;
}) {
  const [revision, setRevision] = useState(0);
  const [state, setState] = useState<Omit<WorkerState, "reset">>({
    status: "starting",
    client: null,
    error: null,
    session: 0,
  });
  useEffect(() => {
    let active = true;
    const session = ++nextSession;
    let client: FinstackClient | undefined;
    setState({ status: "starting", client: null, error: null, session });
    try {
      client = createClient(
        new Worker(
          new URL("../../../workers/finstack.worker.ts", import.meta.url),
          { type: "module" },
        ),
        (error) => {
          if (active)
            setState({ status: "error", client: null, error, session });
          client?.close();
        },
      );
      const current = client;
      current
        .call("initialize", wasmUrl)
        .then(() => {
          if (active)
            setState({
              status: "ready",
              client: current,
              error: null,
              session,
            });
        })
        .catch((error) => {
          if (active)
            setState({
              status: "error",
              client: null,
              error:
                error instanceof FinstackError
                  ? error
                  : new FinstackError(errorValue(error)),
              session,
            });
          current.close();
        });
    } catch (error) {
      setState({
        status: "error",
        client: null,
        error:
          error instanceof FinstackError
            ? error
            : new FinstackError(errorValue(error)),
        session,
      });
    }
    return () => {
      active = false;
      client?.close();
    };
  }, [revision, wasmUrl]);
  return (
    <Context.Provider
      value={{ ...state, reset: () => setRevision((value) => value + 1) }}
    >
      {children}
    </Context.Provider>
  );
}
/** Convenience boundary for a standalone workbench; applications may supply their own query provider. */
export function FinstackQueryProvider({
  children,
  wasmUrl,
}: {
  children: ReactNode;
  wasmUrl?: string;
}) {
  const [client] = useState(() => new QueryClient());
  return (
    <QueryClientProvider client={client}>
      <FinstackProvider wasmUrl={wasmUrl}>{children}</FinstackProvider>
    </QueryClientProvider>
  );
}
export function useFinstack(): WorkerState {
  const state = useContext(Context);
  if (!state) throw new Error("useFinstack requires FinstackProvider");
  return state;
}
