//! Dynamic floating-coupon extraction and rate-model compilation for `ReplayTemplate`.

use super::super::*;
use super::TemplateInputs;

impl ReplayTemplate {
    /// Floating coupons re-fixed on the sampled rate path, and the schedule
    /// indices they replace.
    pub(super) fn build_floating(
        inputs: &TemplateInputs<'_>,
        include_as_of_option_cash: bool,
    ) -> Result<(Vec<FloatingCoupon>, BTreeSet<usize>)> {
        let TemplateInputs {
            tree,
            bond,
            market,
            as_of,
            times,
            step_dates,
            schedule,
            discount,
            final_redemption_date,
        } = *inputs;
        let grid_day_count = DayCount::Act365F;
        let terminal_step = times.len() - 1;
        let floating_spec = bond.cashflow_spec.floating_coupon_spec();
        // One group per accrual interval: a split coupon's cash row and its
        // PIK row share the interval, and the PIK row is dated at its
        // capitalization rather than the cash payment (record
        // CORE-CONVENTIONS-015 #6).
        let mut dynamic_groups = BTreeMap::<(Date, Date), FloatingBuild>::new();
        let mut dynamic_flow_indices = BTreeSet::new();
        if let Some(spec) = floating_spec {
            for (flow_index, flow) in schedule.get_flows().iter().enumerate() {
                if !matches!(flow.get_kind(), CFKind::Floating | CFKind::Pik) {
                    continue;
                }
                let Some(accrual) = &flow.get_accrual() else {
                    continue;
                };
                let dynamic = flow.get_date() > as_of
                    || (flow.get_date() == as_of && include_as_of_option_cash);
                if !dynamic {
                    if flow.get_kind() == CFKind::Pik
                        && flow
                            .get_balance_date()
                            .is_some_and(|balance| balance >= as_of)
                        && spec
                            .rate_spec
                            .compounding
                            .is_some_and(|method| method.is_overnight())
                    {
                        // Excluding settled cash at or before the origin does
                        // not make future PIK fixing risk deterministic. Retain
                        // static PIK only for observed or carry-determined rates.
                        let (overnight, _) = build_overnight_coupon(
                            tree,
                            market,
                            discount.as_ref(),
                            as_of,
                            times,
                            (accrual.get_start(), accrual.get_end()),
                            spec,
                        )?;
                        if overnight
                            .sources
                            .values()
                            .any(|source| matches!(source, OvernightRateSource::Conditional(_)))
                        {
                            return Err(Error::Validation(format!(
                                "Bond '{}' overnight PIK capitalizing on {} still requires unobserved fixings",
                                bond.id.as_str(),
                                flow.get_date(),
                            )));
                        }
                    }
                    continue;
                }
                if accrual.get_projected_index_rate().is_none() {
                    continue;
                }
                let reset = flow
                    .get_reset_date()
                    .unwrap_or(contractual_reset_date(accrual.get_start(), spec)?);
                let key = (accrual.get_start(), accrual.get_end());
                let entry = dynamic_groups.entry(key).or_default();
                entry.reset = Some(reset);
                entry.start = Some(accrual.get_start());
                entry.end = Some(accrual.get_end());
                // The cash row's date is the coupon payment; a PIK-only coupon
                // has no cash settlement and pays nothing before capitalizing.
                if flow.get_kind() != CFKind::Pik || entry.payment.is_none() {
                    entry.payment = Some(flow.get_date());
                }
                // Only the shares still ahead of the origin replay: a cash
                // share settled before it is not paid again at capitalization,
                // and a capitalized PIK share is not added again.
                if flow.get_kind() == CFKind::Pik {
                    entry.has_pik = true;
                } else {
                    entry.has_cash = true;
                }
                entry.day_count = Some(accrual.get_day_count());
                entry.day_count_context = Some(DayCountContext {
                    calendar: Some(calendar_by_id_strict(
                        accrual
                            .get_context()
                            .get_calendar_id()
                            .unwrap_or(&spec.schedule.calendar_id),
                    )?),
                    frequency: Some(spec.schedule.frequency),
                    bus_basis: None,
                    coupon_period: accrual.get_context().get_coupon_period(),
                    end_is_termination_date: accrual.get_context().get_end_is_termination_date(),
                });
                entry.accrual = flow.get_accrual_factor();
                entry.base_index_rate = accrual.get_projected_index_rate();
                dynamic_flow_indices.insert(flow_index);
            }
        }

        if floating_spec.is_some_and(|spec| {
            spec.rate_spec
                .compounding
                .is_some_and(|method| method.is_overnight())
        }) {
            let mut periods_by_payment = BTreeMap::<Date, usize>::new();
            for payment in dynamic_groups.values().filter_map(|group| group.payment) {
                *periods_by_payment.entry(payment).or_default() += 1;
            }
            if let Some((&payment, _)) = periods_by_payment.iter().find(|(_, count)| **count > 1) {
                return Err(Error::Validation(format!(
                    "Bond '{}' segmented overnight coupon paid {} cannot be replayed without its original full-period observation and constraint scope",
                    bond.id.as_str(),
                    payment,
                )));
            }
        }

        let (cash_fraction, pik_fraction) = floating_spec
            .map(|spec| coupon_fractions(spec.coupon_type))
            .transpose()?
            .unwrap_or((0.0, 0.0));
        let params = floating_spec
            .map(|spec| {
                finstack_quant_cashflows::builder::FloatingRateParams::try_from(&spec.rate_spec)
            })
            .transpose()?
            .unwrap_or_default();
        params.validate()?;

        let mut floating = Vec::with_capacity(dynamic_groups.len());
        for group in dynamic_groups.into_values() {
            let reset = required_date(group.reset, "reset")?;
            let start = required_date(group.start, "accrual start")?;
            let end = required_date(group.end, "accrual end")?;
            let payment = required_date(group.payment, "payment")?;
            let day_count = group.day_count.ok_or_else(|| {
                Error::Validation("floating coupon is missing day-count metadata".to_string())
            })?;
            if !group.accrual.is_finite() || group.accrual <= 0.0 {
                return Err(Error::Validation(format!(
                    "Bond '{}' floating coupon {} to {} has invalid accrual factor {}",
                    bond.id.as_str(),
                    start,
                    end,
                    group.accrual
                )));
            }
            let reset_time = if reset <= as_of {
                0.0
            } else {
                grid_day_count.year_fraction(as_of, reset, DayCountContext::default())?
            };
            let start_time = if start <= as_of {
                0.0
            } else {
                grid_day_count.year_fraction(as_of, start, DayCountContext::default())?
            };
            let reset_step = nearest_step(times, reset_time);
            let accrual_start_step = nearest_step(times, start_time);
            let payment_step = exact_grid_step(times, as_of, payment)?;
            if accrual_start_step >= payment_step {
                return Err(Error::Validation(format!(
                    "Bond '{}' floating coupon accruing {} to {} (paid {}) maps accrual start and payment to step {}; use a daily rates-credit grid through the adjusted payment date",
                    bond.id.as_str(),
                    start,
                    end,
                    payment,
                    accrual_start_step
                )));
            }
            let spec = floating_spec.ok_or_else(|| {
                Error::Internal(
                    "bond hazard LSMC dynamic floating coupon has no specification".into(),
                )
            })?;
            let base_index_rate = group.base_index_rate.ok_or_else(|| {
                Error::Validation(format!(
                    "Bond '{}' floating coupon {} to {} is missing projected_index_rate metadata required by hazard LSMC",
                    bond.id.as_str(), start, end
                ))
            })?;
            let (rate_model, observation) = if spec.rate_spec.compounding.is_some() {
                let (overnight_model, overnight_observation) = build_overnight_coupon(
                    tree,
                    market,
                    discount.as_ref(),
                    as_of,
                    times,
                    (start, end),
                    spec,
                )?;
                (
                    FloatingRateModel::Overnight(overnight_model),
                    overnight_observation,
                )
            } else {
                let forward = market.get_forward(&spec.rate_spec.forward_curve_id).ok();
                let fixing_id = fixing_series_id(spec.rate_spec.index_id.as_str());
                let fixings = market.get_series(&fixing_id).ok();
                let known_reset = !matches!(
                    observed_index_fixing(reset, fixings, spec.rate_spec.index_id.as_str(), as_of,),
                    Ok(None)
                );
                let source = match forward.as_ref() {
                    Some(curve) if !known_reset => {
                        let index_tenor = spec
                            .rate_spec
                            .index_tenor
                            .or_else(|| curve.index_tenor())
                            .unwrap_or(spec.rate_spec.reset_frequency);
                        let tenor_years = index_tenor.to_years_with_context(
                            reset,
                            None,
                            finstack_quant_core::dates::BusinessDayConvention::Unadjusted,
                            curve.day_count(),
                            curve.day_count_context(),
                        )?;
                        let observation_end_time = reset_time + tenor_years;
                        if observation_end_time > times[terminal_step] + 1.0e-12 {
                            return Err(Error::Validation(format!(
                                "Bond '{}' term-index observation at {} with tenor {:.12}y ends beyond the calibrated rates-credit horizon {:.12}y",
                                bond.id.as_str(),
                                reset,
                                tenor_years,
                                times[terminal_step]
                            )));
                        }
                        let observation_end_step = nearest_step(times, observation_end_time);
                        if observation_end_step <= reset_step {
                            return Err(Error::Validation(format!(
                                "Bond '{}' term-index observation at {} with tenor {:.12}y maps to no positive rates-credit interval",
                                bond.id.as_str(),
                                reset,
                                tenor_years
                            )));
                        }
                        let index_accrual = times[observation_end_step] - times[reset_step];
                        let reset_grid_date = step_dates[reset_step].ok_or_else(|| {
                            Error::Validation(format!(
                                "Bond '{}' term-index reset step {} has no calendar date",
                                bond.id.as_str(),
                                reset_step
                            ))
                        })?;
                        let observation_end_date =
                            step_dates[observation_end_step].ok_or_else(|| {
                                Error::Validation(format!(
                                "Bond '{}' term-index observation-end step {} has no calendar date",
                                bond.id.as_str(),
                                observation_end_step
                            ))
                            })?;
                        let base_df =
                            discount.df_between_dates(reset_grid_date, observation_end_date)?;
                        if !base_df.is_finite() || base_df <= 0.0 {
                            return Err(Error::Validation(format!(
                                "Bond '{}' has an invalid reference discount factor over the term-index observation [{:.12}, {:.12}]",
                                bond.id.as_str(),
                                times[reset_step],
                                times[observation_end_step]
                            )));
                        }
                        ObservedRateSource::Conditional(ConditionalRate {
                            observation_step: reset_step,
                            accrual: index_accrual,
                            base_index_rate,
                            base_discount_forward: (1.0 / base_df - 1.0) / index_accrual,
                            conditional_discount_factors: tree
                                .conditional_discount_factors(reset_step, observation_end_step)?,
                        })
                    }
                    _ => ObservedRateSource::Fixed(base_index_rate),
                };
                (
                    FloatingRateModel::Term(source),
                    FloatingRateObservation::Term { reset_date: reset },
                )
            };
            let compiled = CompiledFloatingCoupon::compile(
                FloatingCouponPeriod {
                    accrual_start: start,
                    accrual_end: end,
                    payment_date: payment,
                    day_count,
                    day_count_context: group.day_count_context.ok_or_else(|| {
                        Error::Internal("floating coupon is missing day-count context".into())
                    })?,
                    accrual_factor: group.accrual,
                },
                FloatingCouponEconomics {
                    cash_fraction,
                    pik_fraction,
                    rate_params: params.clone(),
                },
                observation,
            )?;
            if let FloatingRateModel::Overnight(overnight) = &rate_model {
                for source in overnight.sources.values() {
                    if let OvernightRateSource::Conditional(source) = source
                        && source.observation_step > payment_step
                    {
                        return Err(Error::Validation(format!(
                            "Bond '{}' overnight coupon paid {} requires an observation after payment; adjust the accrual end or payment lag",
                            bond.id.as_str(),
                            payment,
                        )));
                    }
                }
            }
            let initial_notional = (start < as_of)
                .then(|| {
                    scheduled_outstanding_after(
                        bond,
                        schedule.get_flows(),
                        start,
                        final_redemption_date,
                    )
                })
                .transpose()?;
            let initial_term_rate = match &rate_model {
                FloatingRateModel::Term(ObservedRateSource::Fixed(rate)) if reset <= as_of => {
                    Some(*rate)
                }
                _ => None,
            };
            floating.push(FloatingCoupon {
                reset_step,
                accrual_start_step,
                payment_step,
                effective_end_step: if end < as_of {
                    0
                } else {
                    exact_grid_step(times, as_of, end)?
                },
                pays_cash: group.has_cash && cash_fraction > 0.0,
                capitalizes: group.has_pik && pik_fraction > 0.0,
                compiled,
                rate_model,
                initial_notional,
                initial_term_rate,
            });
        }

        Ok((floating, dynamic_flow_indices))
    }
}
