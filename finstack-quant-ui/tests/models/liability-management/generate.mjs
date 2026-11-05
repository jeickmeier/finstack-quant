import { emit } from "../../../scripts/fixture-output.mjs";
import { native } from "../../../scripts/fixture-inputs.mjs";
// The worker's ModelsApi over the raw package (the facade's
// models.credit.liabilityManagement namespace re-exports these unchanged).
const exchange = (request) => native.analyzeExchangeOffer(request);
const lme = (request) => native.analyzeLme(request);
const attempt = (call) => {
  try {
    return { ok: true, value: call() };
  } catch (error) {
    return {
      ok: false,
      error: { name: error.name, message: error.message, kind: error.kind },
    };
  }
};
const entry = (analyze, request) => ({
  request,
  result: attempt(() => analyze(request)),
});
const cases = {
  hurdle: native.tenderRecommendationHurdle(),
  exchange: {
    // A discounted exchange worth clearly more than holding out.
    recommended: entry(exchange, {
      old_pv: 45,
      new_pv: 80,
      consent_fee: 2,
      equity_sweetener_value: 3,
      exchange_type: "discount",
    }),
    // Consideration above the hold-out value, but not by more than the hurdle.
    insideHurdle: entry(exchange, {
      old_pv: 100,
      new_pv: 100.5,
      consent_fee: 0.5,
      equity_sweetener_value: 0.25,
      exchange_type: "par_for_par",
    }),
    negativeFee: entry(exchange, {
      old_pv: 45,
      new_pv: 80,
      consent_fee: -1,
      equity_sweetener_value: 0,
      exchange_type: "uptier",
    }),
  },
  lme: {
    tender: entry(lme, {
      lme_type: "tender_offer",
      notional: 1000,
      repurchase_price_pct: 0.6,
      opt_acceptance_pct: 0.8,
      ebitda: 125,
    }),
    amendAndExtend: entry(lme, {
      lme_type: "amend_and_extend",
      notional: 1000,
      repurchase_price_pct: 0.01,
      opt_acceptance_pct: 0.9,
      ebitda: null,
    }),
    dropdown: entry(lme, {
      lme_type: "dropdown",
      notional: 1000,
      repurchase_price_pct: 0.3,
      opt_acceptance_pct: 0.7,
      ebitda: 125,
    }),
    pricePoints: entry(lme, {
      lme_type: "open_market_repurchase",
      notional: 1000,
      repurchase_price_pct: 60,
      opt_acceptance_pct: 0.8,
      ebitda: null,
    }),
  },
};
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    description:
      "Native models.credit.liabilityManagement results: exchange offers above and inside the tender hurdle, tender, amend-and-extend and dropdown exercises, and the validation errors of a negative fee and a price quoted in points.",
    ...cases,
  }),
);
