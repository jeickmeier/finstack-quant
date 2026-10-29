import altmanZScore from "@/lib/finstack/generated/schemas/altman_z_score_input.json";
import altmanZPrime from "@/lib/finstack/generated/schemas/altman_z_prime_input.json";
import altmanZDoublePrime from "@/lib/finstack/generated/schemas/altman_z_double_prime_input.json";
import ohlsonOScore from "@/lib/finstack/generated/schemas/ohlson_o_score_input.json";
import zmijewski from "@/lib/finstack/generated/schemas/zmijewski_input.json";
/** A generated scoring input contract: its fields in Rust declaration order, with their Rust descriptions. */
export interface ScoringContract {
  description: string;
  required: readonly string[];
  properties: Readonly<Record<string, { description?: string }>>;
}
/**
 * The models.credit.scoring functions the scorecard offers, each with the
 * generated input contract of its ratios; altmanEmScore takes the Z''-Score
 * ratios. The labels are this editor's own control choices.
 */
export const scoringModels = [
  { model: "altmanZScore", label: "Altman Z (1968)", contract: altmanZScore },
  {
    model: "altmanZPrime",
    label: "Altman Z' (private)",
    contract: altmanZPrime,
  },
  {
    model: "altmanZDoublePrime",
    label: "Altman Z'' (non-manufacturer)",
    contract: altmanZDoublePrime,
  },
  {
    model: "altmanEmScore",
    label: "Altman EM (1995)",
    contract: altmanZDoublePrime,
  },
  { model: "ohlsonOScore", label: "Ohlson O (1980)", contract: ohlsonOScore },
  { model: "zmijewskiScore", label: "Zmijewski (1984)", contract: zmijewski },
] as const satisfies readonly {
  model: string;
  label: string;
  contract: ScoringContract;
}[];
