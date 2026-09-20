/** Immutable request plus a key that keeps invalid non-finite native inputs distinct from null. */
export function requestSnapshot<T extends object>(request: T | null) {
  const snapshot = request === null ? null : structuredClone(request);
  const key = JSON.stringify(snapshot, (_name, value) =>
    typeof value === "number" && !Number.isFinite(value)
      ? { nonFinite: String(value) }
      : value,
  );
  return { snapshot, key };
}
