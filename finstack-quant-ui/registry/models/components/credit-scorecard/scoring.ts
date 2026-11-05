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
 * The Rust CreditScoreRequest models the scorecard offers, each with the
 * generated input contract of its ratios; altman_em_score takes the Z''-Score
 * ratios. The labels are this editor's own control choices.
 */
export const scoringModels = [
  { model: "altman_z_score", label: "Altman Z (1968)", contract: altmanZScore },
  {
    model: "altman_z_prime",
    label: "Altman Z' (private)",
    contract: altmanZPrime,
  },
  {
    model: "altman_z_double_prime",
    label: "Altman Z'' (non-manufacturer)",
    contract: altmanZDoublePrime,
  },
  {
    model: "altman_em_score",
    label: "Altman EM (1995)",
    contract: altmanZDoublePrime,
  },
  { model: "ohlson_o_score", label: "Ohlson O (1980)", contract: ohlsonOScore },
  { model: "zmijewski_score", label: "Zmijewski (1984)", contract: zmijewski },
] as const satisfies readonly {
  model: string;
  label: string;
  contract: ScoringContract;
}[];
