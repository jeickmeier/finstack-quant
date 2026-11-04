import { vi } from "vitest";

/**
 * Stand-in for `@/hooks/shared/use-finstack/client`. vi.mock is hoisted only in
 * the file that calls it, so each test file declares
 * `vi.mock("@/hooks/shared/use-finstack/client", () => import("<relative>/shared/worker/client"))`
 * and then sets what this returns, directly or through `useWorkerHarness`.
 */
export const createClient = vi.fn();
