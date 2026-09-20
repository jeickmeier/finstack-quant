import { valuationCodec, type ValuationResult } from "../../src/host";
/** Restore only transport kinds recorded from the actual facade, never guessed domain fields. */
export function restore(entry: {
  resultJson: string;
  hostBigIntPaths: (string | number)[][];
}): ValuationResult {
  const result = valuationCodec.parse(entry.resultJson);
  for (const path of entry.hostBigIntPaths) {
    let parent = result as Record<string | number, unknown>;
    for (const key of path.slice(0, -1))
      parent = parent[key] as Record<string | number, unknown>;
    const key = path.at(-1)!;
    if (typeof parent[key] === "bigint") continue;
    if (!Number.isSafeInteger(parent[key]))
      throw new TypeError("Unsafe fixture integer");
    parent[key] = BigInt(parent[key] as number);
  }
  return result as ValuationResult;
}
