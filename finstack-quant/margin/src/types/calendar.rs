//! Default regulatory holiday-calendar mapping for margin currencies.

pub(super) fn default_margin_calendar(
    currency: finstack_quant_core::currency::Currency,
) -> &'static str {
    use finstack_quant_core::currency::Currency;

    match currency {
        Currency::USD => "usny",
        Currency::EUR => "target2",
        Currency::GBP => "gblo",
        Currency::JPY => "jpto",
        Currency::CHF => "chzh",
        Currency::CAD => "cato",
        Currency::AUD => "auce",
        _ => "weekends_only",
    }
}

#[cfg(test)]
mod tests {
    use super::default_margin_calendar;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn default_margin_calendars_cover_regulatory_currencies_and_fallback() {
        assert_eq!(default_margin_calendar(Currency::USD), "usny");
        assert_eq!(default_margin_calendar(Currency::EUR), "target2");
        assert_eq!(default_margin_calendar(Currency::GBP), "gblo");
        assert_eq!(default_margin_calendar(Currency::JPY), "jpto");
        assert_eq!(default_margin_calendar(Currency::CHF), "chzh");
        assert_eq!(default_margin_calendar(Currency::CAD), "cato");
        assert_eq!(default_margin_calendar(Currency::AUD), "auce");
        assert_eq!(default_margin_calendar(Currency::NZD), "weekends_only");
    }
}
