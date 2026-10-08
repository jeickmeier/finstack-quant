# Bond Future

Exchange bond future on a deliverable basket. Pricing marks the
**caller-supplied CTD**; it does not rank the basket.

## Conventions

- Set the CTD on the instrument (`ctd_bond_id` plus the embedded `ctd_bond`)
  and refresh that choice when the basket can switch. `Instrument::value` will
  not search.
- The model price is the carry-adjusted forward clean CTD price divided by the
  conversion factor. Variation-margin futures are not discounted further.

Import path:
`finstack_quant_valuations::instruments::fixed_income::bond_future`
(`BondFuture` is also re-exported at `finstack_quant_valuations::instruments`).
