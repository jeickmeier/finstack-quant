import { emit } from "../../../scripts/fixture-output.mjs";
import { native } from "../../../scripts/fixture-inputs.mjs";
// The worker's ModelsApi over the raw package (the facade's
// models.credit.recoveryWaterfall and models.credit.lgd namespaces re-export
// these unchanged). The WorkoutLgd and BetaRecovery handles are read the way
// the worker reads them.
const allocate = (request) =>
  native.allocateRecovery(request.estateValue, request.claims);
const workout = (request) => {
  const model = native.WorkoutLgd.fromJson(request.model);
  try {
    return model.evaluate(request.ead);
  } finally {
    model.free();
  }
};
const piece = (collateral_type, book_value, haircut) => ({
  collateral_type,
  book_value,
  haircut,
});
const terms = {
  costs: { direct_cost_rate: 0.05, indirect_cost_rate: 0.03 },
  workout_years: 2,
  discount_rate: 0.06,
};
const stats = (request) => {
  const recovery = native.seniorityRecoveryStats(
    request.seniority,
    request.calibrationId,
  );
  try {
    return {
      mean: recovery.mean,
      std_dev: recovery.stdDev,
      variance: recovery.variance,
      alpha: recovery.alpha,
      beta_param: recovery.betaParam,
      mode: recovery.mode,
      mean_lgd: recovery.meanLgd,
    };
  } finally {
    recovery.free();
  }
};
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
const claim = (id, seniority, priority, principal, collateral = {}) => ({
  id,
  seniority,
  priority,
  principal,
  accrued: 0,
  penalties: 0,
  collateral_value: null,
  collateral_haircut: 0,
  ...collateral,
});
// A secured term loan over its own collateral, two pari passu notes that share
// a shortfall, and subordinated notes the estate does not reach.
const stack = [
  claim("TL-B", "senior_secured", 1, 100, {
    accrued: 2,
    collateral_value: 80,
    collateral_haircut: 0.25,
  }),
  claim("Notes-A", "senior_unsecured", 2, 60),
  claim("Notes-B", "senior_unsecured", 2, 40),
  claim("Sub-Notes", "subordinated", 3, 50),
];
const cases = {
  calibrationIds: native.seniorityCalibrationIds(),
  waterfall: {
    shortfall: entry(allocate, { estateValue: 150, claims: stack }),
    surplus: entry(allocate, { estateValue: 300, claims: stack }),
    collateralAboveEstate: entry(allocate, { estateValue: 50, claims: stack }),
    duplicateId: entry(allocate, {
      estateValue: 150,
      claims: [stack[1], { ...stack[2], id: "Notes-A" }],
    }),
  },
  workout: {
    secured: entry(workout, {
      ead: 1_000_000,
      model: {
        collateral: [
          piece("real_estate", 600_000, 0.3),
          piece("receivables", 200_000, 0.35),
        ],
        ...terms,
      },
    }),
    unknownCollateral: entry(workout, {
      ead: 1_000_000,
      model: { collateral: [piece("real-estate", 600_000, 0.3)], ...terms },
    }),
  },
  stats: {
    seniorUnsecured: entry(stats, {
      seniority: "senior_unsecured",
      calibrationId: "moodys_recovery_1982_2023",
    }),
    classNotCalibrated: entry(stats, {
      seniority: "1st_lien_secured",
      calibrationId: "sp_recovery_historical",
    }),
    unknownCalibration: entry(stats, {
      seniority: "senior_secured",
      calibrationId: "moodys",
    }),
  },
};
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    description:
      "Native models.credit.recoveryWaterfall.allocateRecovery, models.credit.lgd.WorkoutLgd.fromJson(model).evaluate(ead), seniorityRecoveryStats (read from the BetaRecovery handle) and seniorityCalibrationIds results: a four-claim stack with a pari passu shortfall and with a surplus, net collateral above the estate, a repeated claim id, a two-piece workout and a non-canonical collateral type, and statistics for a calibrated class, a class the S&P calibration lacks and an unknown calibration id.",
    ...cases,
  }),
);
