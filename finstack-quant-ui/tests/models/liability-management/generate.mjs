import { emit } from "../../../scripts/fixture-output.mjs";
import { native } from "../../../scripts/fixture-inputs.mjs";
// The worker's ModelsApi over the raw package (the facade's
// models.credit.liabilityManagement namespace re-exports these unchanged).
const exchange = (request) =>
  native.analyzeExchangeOffer(
    request.oldPv,
    request.newPv,
    request.consentFee,
    request.equitySweetenerValue,
    request.exchangeType,
  );
const lme = (request) =>
  native.analyzeLme(
    request.lmeType,
    request.notional,
    request.repurchasePricePct,
    request.optAcceptancePct,
    request.ebitda,
  );
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
      oldPv: 45,
      newPv: 80,
      consentFee: 2,
      equitySweetenerValue: 3,
      exchangeType: "discount",
    }),
    // Consideration above the hold-out value, but not by more than the hurdle.
    insideHurdle: entry(exchange, {
      oldPv: 100,
      newPv: 100.5,
      consentFee: 0.5,
      equitySweetenerValue: 0.25,
      exchangeType: "par_for_par",
    }),
    negativeFee: entry(exchange, {
      oldPv: 45,
      newPv: 80,
      consentFee: -1,
      equitySweetenerValue: 0,
      exchangeType: "uptier",
    }),
  },
  lme: {
    tender: entry(lme, {
      lmeType: "tender_offer",
      notional: 1000,
      repurchasePricePct: 0.6,
      optAcceptancePct: 0.8,
      ebitda: 125,
    }),
    amendAndExtend: entry(lme, {
      lmeType: "amend_and_extend",
      notional: 1000,
      repurchasePricePct: 0.01,
      optAcceptancePct: 0.9,
      ebitda: null,
    }),
    dropdown: entry(lme, {
      lmeType: "dropdown",
      notional: 1000,
      repurchasePricePct: 0.3,
      optAcceptancePct: 0.7,
      ebitda: 125,
    }),
    pricePoints: entry(lme, {
      lmeType: "open_market_repurchase",
      notional: 1000,
      repurchasePricePct: 60,
      optAcceptancePct: 0.8,
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
