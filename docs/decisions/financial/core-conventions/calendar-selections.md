# Calendar data selections at the audited revision

Source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; audit date 2026-11-05. This appendix forms part of [CORE-CONVENTIONS-005](005-jurisdiction-calendars.md). Decision status is Draft; these are observed selections, not adoption approval or refreshed external holiday certification.

All 28 JSON definitions were inspected. Every rule below is consumed by the build generator; dates and effective bounds are parameters, not an independent recalculation. Holiday union, observation and span behavior are specified in [004](004-holiday-engine.md); Hong Kong/Japan procedural expansion is in [005](005-jurisdiction-calendars.md). Saturday/Sunday are nonbusiness for all built-ins. Negative nth-weekday positions count backwards from month-end. Early-close sessions are not separately modeled.

## asx: Australian Securities Exchange (ASX)

Source: [asx.json](../../../../finstack-quant/core/data/calendars/asx.json); `ignore_weekends=false`; 8 rules.

Data-owner scope/provenance note (not externally revalidated): ASX cash-market trading and CHESS settlement calendar. Early-close sessions remain business days. Annual published-date fixtures cover 2024, 2026 and 2027; recurring rules are not certification of all historical or future exchange notices. <https://www.asx.com.au/markets/market-resources/trading-hours-calendar/cash-market-trading-hours/trading-calendar>

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=next_monday |
| Australia Day | 01-26; observed=next_monday |
| Good Friday | Easter Monday -3 calendar days |
| Easter Monday | Easter Monday +0 calendar days |
| ANZAC Day | 04-25; observed=none |
| King's Birthday | 2 occurrence of monday in june |
| Christmas Day | 12-25; observed=mon_if_sat_tue_if_sun |
| Boxing Day | 12-26; observed=mon_if_sat_tue_if_sun |

## auce: Australia (Sydney / New South Wales) Banking Holidays

Source: [auce.json](../../../../finstack-quant/core/data/calendars/auce.json); `ignore_weekends=false`; 10 rules.

Data-owner scope/provenance note (not externally revalidated): Sydney/NSW banking calendar, including the August bank holiday and October Labour Day. It is not a universal Australian state or ASX trading calendar. Modern Australia Day observance is January 26, Mondayized only on weekends; exceptional historical notices require dated maintenance.

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=next_monday |
| Australia Day | 01-26; observed=next_monday |
| Good Friday | Easter Monday -3 calendar days |
| Easter Monday | Easter Monday +0 calendar days |
| ANZAC Day | 04-25; observed=none |
| King's Birthday | 2 occurrence of monday in june |
| Bank Holiday | 1 occurrence of monday in august |
| Labour Day | 1 occurrence of monday in october |
| Christmas Day | 12-25; observed=mon_if_sat_tue_if_sun |
| Boxing Day | 12-26; observed=mon_if_sat_tue_if_sun |

## brbd: Brazil National Banking / Accrual Holidays (ANBIMA)

Source: [brbd.json](../../../../finstack-quant/core/data/calendars/brbd.json); `ignore_weekends=true`; 13 rules.

Data-owner scope/provenance note (not externally revalidated): ANBIMA national banking-holiday convention for business-day accrual. Excludes municipal holidays, elections and year-end customer-service closures; Dec 24 and Dec 31 remain accrual business days when weekdays. This is not the B3 listed-market trading calendar. <https://www.anbima.com.br/feriados/> . Exceptional and historical rule changes outside the published fixtures are not certified.

| Holiday / selection | Implemented parameters |
|---|---|
| Confraternização Universal (New Year's Day) | 01-01; observed=none |
| Tiradentes | 04-21; observed=none |
| Dia do Trabalho (Labor Day) | 05-01; observed=none |
| Independência do Brasil | 09-07; observed=none |
| Nossa Senhora Aparecida | 10-12; observed=none |
| Finados (All Souls' Day) | 11-02; observed=none |
| Proclamação da República | 11-15; observed=none |
| National Black Consciousness Day (national holiday from 2024) | 11-20; observed=none; effective [2024, unbounded] |
| Natal (Christmas) | 12-25; observed=none |
| Sexta-feira Santa (Good Friday) | Easter Monday -3 calendar days |
| Carnaval (Carnival Monday) | Easter Monday -49 calendar days |
| Carnaval (Carnival Tuesday) | Easter Monday -48 calendar days |
| Corpus Christi | Easter Monday +59 calendar days |

## bse: Bombay Stock Exchange (BSE) Holiday Calendar

Source: [bse.json](../../../../finstack-quant/core/data/calendars/bse.json); `ignore_weekends=false`; 44 rules.

Data-owner scope/provenance note (not externally revalidated): BSE cash-segment holidays match NSE. Both IDs exist so desks can look up either name. Recurring Gregorian/statutory rules apply every year. Moving festivals use published circular dates (exact_date) for 2024-2026 only. Years outside that verified window have only the statutory/Gregorian rules. Full-day model: Diwali Laxmi Pujan regular sessions are holidays; Muhurat is not modeled.

| Holiday / selection | Implemented parameters |
|---|---|
| Republic Day | 01-26; observed=none |
| Maharashtra Day | 05-01; observed=none |
| Independence Day | 08-15; observed=none |
| Gandhi Jayanti | 10-02; observed=none |
| Christmas | 12-25; observed=none |
| Good Friday | Easter Monday -3 calendar days |
| Mahashivratri 2024 | 2024-03-08 |
| Holi 2024 | 2024-03-25 |
| Id-Ul-Fitr 2024 | 2024-04-11 |
| Ambedkar Jayanti 2024 | 2024-04-14 |
| Ram Navami 2024 | 2024-04-17 |
| Mahavir Jayanti 2024 | 2024-04-21 |
| Bakri Id 2024 | 2024-06-17 |
| Muharram 2024 | 2024-07-17 |
| Ganesh Chaturthi 2024 | 2024-09-07 |
| Dussehra 2024 | 2024-10-12 |
| Diwali Laxmi Pujan 2024 | 2024-11-01 |
| Diwali-Balipratipada 2024 | 2024-11-02 |
| Guru Nanak Jayanti 2024 | 2024-11-15 |
| Mahashivratri 2025 | 2025-02-26 |
| Holi 2025 | 2025-03-14 |
| Id-Ul-Fitr 2025 | 2025-03-31 |
| Ram Navami 2025 | 2025-04-06 |
| Mahavir Jayanti 2025 | 2025-04-10 |
| Ambedkar Jayanti 2025 | 2025-04-14 |
| Bakri Id 2025 | 2025-06-07 |
| Muharram 2025 | 2025-07-06 |
| Ganesh Chaturthi 2025 | 2025-08-27 |
| Diwali Laxmi Pujan 2025 | 2025-10-21 |
| Diwali-Balipratipada 2025 | 2025-10-22 |
| Guru Nanak Jayanti 2025 | 2025-11-05 |
| Mahashivratri 2026 | 2026-02-15 |
| Holi 2026 | 2026-03-03 |
| Id-Ul-Fitr 2026 | 2026-03-21 |
| Ram Navami 2026 | 2026-03-26 |
| Mahavir Jayanti 2026 | 2026-03-31 |
| Ambedkar Jayanti 2026 | 2026-04-14 |
| Bakri Id 2026 | 2026-05-28 |
| Muharram 2026 | 2026-06-26 |
| Ganesh Chaturthi 2026 | 2026-09-14 |
| Dussehra 2026 | 2026-10-20 |
| Diwali Laxmi Pujan 2026 | 2026-11-08 |
| Diwali-Balipratipada 2026 | 2026-11-10 |
| Guru Nanak Jayanti 2026 | 2026-11-24 |

## cato: Canada (Toronto) Holiday Calendar

Source: [cato.json](../../../../finstack-quant/core/data/calendars/cato.json); `ignore_weekends=false`; 12 rules.

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=next_monday |
| Canada Day | 07-01; observed=next_monday |
| National Day for Truth and Reconciliation | 09-30; observed=next_monday; effective [2021, unbounded] |
| Remembrance Day | 11-11; observed=next_monday |
| Christmas Day | 12-25; observed=mon_if_sat_tue_if_sun |
| Boxing Day | 12-26; observed=mon_if_sat_tue_if_sun |
| Family Day | 3 occurrence of monday in february; effective [2008, unbounded] |
| Good Friday | Easter Monday -3 calendar days |
| Victoria Day | monday on or before 05-24 |
| Civic Holiday | 1 occurrence of monday in august |
| Labour Day | 1 occurrence of monday in september |
| Thanksgiving | 2 occurrence of monday in october |

## chzh: Switzerland (Zurich) Holiday Calendar

Source: [chzh.json](../../../../finstack-quant/core/data/calendars/chzh.json); `ignore_weekends=false`; 10 rules.

Data-owner scope/provenance note (not externally revalidated): Zurich bank holidays: New Year's Day, Berchtoldstag (2 Jan), Good Friday, Easter Monday, Labour Day, Ascension, Whit Monday, Swiss National Day (1 Aug), Christmas and St Stephen's Day. Swiss practice does not move a holiday that falls on a weekend. Same rule set as the QuantLib Switzerland calendar.

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=none |
| New Year's Holiday | 01-02; observed=none |
| Good Friday | Easter Monday -3 calendar days |
| Easter Monday | Easter Monday +0 calendar days |
| Labour Day | 05-01; observed=none |
| Ascension Day | Easter Monday +38 calendar days |
| Whit Monday | Easter Monday +49 calendar days |
| Swiss National Day | 08-01; observed=none |
| Christmas Day | 12-25; observed=none |
| St. Stephen's Day | 12-26; observed=none |

## cme: CME (Chicago Mercantile Exchange) Holidays

Source: [cme.json](../../../../finstack-quant/core/data/calendars/cme.json); `ignore_weekends=false`; 10 rules.

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=fri_if_sat_mon_if_sun |
| Juneteenth National Independence Day | 06-19; observed=fri_if_sat_mon_if_sun; effective [2022, unbounded] |
| Independence Day | 07-04; observed=fri_if_sat_mon_if_sun |
| Christmas Day | 12-25; observed=fri_if_sat_mon_if_sun |
| Martin Luther King Jr. Day | 3 occurrence of monday in january; effective [1998, unbounded] |
| Washington's Birthday | 3 occurrence of monday in february |
| Memorial Day | -1 occurrence of monday in may |
| Labor Day | 1 occurrence of monday in september |
| Thanksgiving Day | 4 occurrence of thursday in november |
| Good Friday | Easter Monday -3 calendar days |

## cnbe: China Mainland (Banking) Holiday Calendar

Source: [cnbe.json](../../../../finstack-quant/core/data/calendars/cnbe.json); `ignore_weekends=false`; 50 rules.

Data-owner scope/provenance note (not externally revalidated): Mainland China banking calendar. Spring Festival and Labour Day use published year-specific closure spans for 2004-2026; rules outside that verified range are explicitly estimates. Single-day statutory festivals use the modern 3-day 连休 (china_bridge) convention codified by the State Council (Nov 2024, effective 2025).

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | China bridge around (01-01; observed=none) |
| Estimated Spring Festival (pre-verified coverage) | 7 consecutive days from (chinese_new_year), offset +0; effective [unbounded, 2003] |
| Spring Festival 2004 | 7 consecutive days from (01-22; observed=none), offset +0; effective [2004, 2004] |
| Spring Festival 2005 | 7 consecutive days from (02-09; observed=none), offset +0; effective [2005, 2005] |
| Spring Festival 2006 | 7 consecutive days from (01-29; observed=none), offset +0; effective [2006, 2006] |
| Spring Festival 2007 | 7 consecutive days from (02-18; observed=none), offset +0; effective [2007, 2007] |
| Spring Festival 2008 | 7 consecutive days from (02-06; observed=none), offset +0; effective [2008, 2008] |
| Spring Festival 2009 | 7 consecutive days from (01-25; observed=none), offset +0; effective [2009, 2009] |
| Spring Festival 2010 | 7 consecutive days from (02-13; observed=none), offset +0; effective [2010, 2010] |
| Spring Festival 2011 | 7 consecutive days from (02-02; observed=none), offset +0; effective [2011, 2011] |
| Spring Festival 2012 | 7 consecutive days from (01-22; observed=none), offset +0; effective [2012, 2012] |
| Spring Festival 2013 | 7 consecutive days from (02-09; observed=none), offset +0; effective [2013, 2013] |
| Spring Festival 2014 | 7 consecutive days from (01-31; observed=none), offset +0; effective [2014, 2014] |
| Spring Festival 2015 | 7 consecutive days from (02-18; observed=none), offset +0; effective [2015, 2015] |
| Spring Festival 2016 | 7 consecutive days from (02-07; observed=none), offset +0; effective [2016, 2016] |
| Spring Festival 2017 | 7 consecutive days from (01-27; observed=none), offset +0; effective [2017, 2017] |
| Spring Festival 2018 | 7 consecutive days from (02-15; observed=none), offset +0; effective [2018, 2018] |
| Spring Festival 2019 | 7 consecutive days from (02-04; observed=none), offset +0; effective [2019, 2019] |
| Spring Festival 2020 extended closure | 10 consecutive days from (01-24; observed=none), offset +0; effective [2020, 2020] |
| Spring Festival 2021 | 7 consecutive days from (02-11; observed=none), offset +0; effective [2021, 2021] |
| Spring Festival 2022 | 7 consecutive days from (01-31; observed=none), offset +0; effective [2022, 2022] |
| Spring Festival 2023 | 7 consecutive days from (01-21; observed=none), offset +0; effective [2023, 2023] |
| Spring Festival 2024 | 9 consecutive days from (02-09; observed=none), offset +0; effective [2024, 2024] |
| Spring Festival 2025 | 8 consecutive days from (01-28; observed=none), offset +0; effective [2025, 2025] |
| Spring Festival 2026 | 9 consecutive days from (02-15; observed=none), offset +0; effective [2026, 2026] |
| Estimated Spring Festival (post-verified coverage) | 8 consecutive days from (chinese_new_year), offset -1; effective [2027, unbounded] |
| Qingming Festival | China bridge around (qing_ming); effective [2008, unbounded] |
| Estimated Labour Day Golden Week (pre-verified coverage) | 7 consecutive days from (05-01; observed=none), offset +0; effective [unbounded, 2003] |
| Labour Day Golden Week 2004-2007 | 7 consecutive days from (05-01; observed=none), offset +0; effective [2004, 2007] |
| Labour Day 2008-2010 | 3 consecutive days from (05-01; observed=none), offset +0; effective [2008, 2010] |
| Labour Day 2011 | 3 consecutive days from (04-30; observed=none), offset +0; effective [2011, 2011] |
| Labour Day 2012 | 3 consecutive days from (04-29; observed=none), offset +0; effective [2012, 2012] |
| Labour Day 2013 | 3 consecutive days from (04-29; observed=none), offset +0; effective [2013, 2013] |
| Labour Day 2014-2015 | 3 consecutive days from (05-01; observed=none), offset +0; effective [2014, 2015] |
| Labour Day 2016 | 3 consecutive days from (04-30; observed=none), offset +0; effective [2016, 2016] |
| Labour Day 2017 | 3 consecutive days from (04-29; observed=none), offset +0; effective [2017, 2017] |
| Labour Day 2018 | 3 consecutive days from (04-29; observed=none), offset +0; effective [2018, 2018] |
| Labour Day 2019 | 4 consecutive days from (05-01; observed=none), offset +0; effective [2019, 2019] |
| Labour Day 2020-2021 | 5 consecutive days from (05-01; observed=none), offset +0; effective [2020, 2021] |
| Labour Day 2022 | 5 consecutive days from (04-30; observed=none), offset +0; effective [2022, 2022] |
| Labour Day 2023 | 5 consecutive days from (04-29; observed=none), offset +0; effective [2023, 2023] |
| Labour Day 2024-2026 | 5 consecutive days from (05-01; observed=none), offset +0; effective [2024, 2026] |
| Estimated Labour Day (post-verified coverage) | 5 consecutive days from (05-01; observed=none), offset +0; effective [2027, unbounded] |
| Dragon Boat Festival | China bridge around (dragon_boat); effective [2008, unbounded] |
| Mid-Autumn Festival | China bridge around (mid_autumn); effective [2008, unbounded] |
| National Day Golden Week | 7 consecutive days from (10-01; observed=none), offset +0 |
| National Day Golden Week 2009 extended closure | 2009-10-08 |
| Mid-Autumn Festival 2010 published closure tail | 2 consecutive days from (09-23; observed=none), offset +0; effective [2010, 2010] |
| 70th anniversary commemoration closure | 2 consecutive days from (09-03; observed=none), offset +0; effective [2015, 2015] |
| National Day / Mid-Autumn merged 8th day (2025) | 2025-10-08 |

## defr: Germany (Frankfurt) Holiday Calendar

Source: [defr.json](../../../../finstack-quant/core/data/calendars/defr.json); `ignore_weekends=false`; 6 rules.

Data-owner scope/provenance note (not externally revalidated): Same holiday set as TARGET2 (New Year's Day, Good Friday, Easter Monday, Labour Day, Christmas, St Stephen's Day), as noted in eurex.json. German practice does not move a holiday that falls on a weekend.

| Holiday / selection | Implemented parameters |
|---|---|
| Neujahr (New Year's Day) | 01-01; observed=none |
| Karfreitag (Good Friday) | Easter Monday -3 calendar days |
| Ostermontag (Easter Monday) | Easter Monday +0 calendar days |
| Tag der Arbeit (Labour Day) | 05-01; observed=none |
| Erster Weihnachtstag (Christmas Day) | 12-25; observed=none |
| Zweiter Weihnachtstag (St. Stephen's Day) | 12-26; observed=none |

## eurex: Eurex Exchange Holiday Calendar

Source: [eurex.json](../../../../finstack-quant/core/data/calendars/eurex.json); `ignore_weekends=false`; 8 rules.

Data-owner scope/provenance note (not externally revalidated): TARGET2-style all-derivatives closes plus Christmas Eve and New Year's Eve (trading-only closes). This is the gap versus defr/target2. Source: Eurex holiday regulations (full-day model; no half-day / early-close representation).

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=none |
| Good Friday | Easter Monday -3 calendar days |
| Easter Monday | Easter Monday +0 calendar days |
| Labour Day | 05-01; observed=none |
| Christmas Eve | 12-24; observed=none |
| Christmas Day | 12-25; observed=none |
| Boxing Day | 12-26; observed=none |
| New Year's Eve | 12-31; observed=none |

## gblo: United Kingdom (London) Holiday Calendar

Source: [gblo.json](../../../../finstack-quant/core/data/calendars/gblo.json); `ignore_weekends=false`; 18 rules.

Data-owner scope/provenance note (not externally revalidated): Christmas/Boxing Day use chained substitution (mon_if_sat_tue_if_sun): each moves two days when on a weekend so the two observed days never collide (2021: Mon 27 + Tue 28 Dec; 2022: Mon 26 + Tue 27 Dec). One-off bank holidays and moved May bank holidays per UK government history (gov.uk/bank-holidays): 2012 Diamond Jubilee (Spring BH moved to Jun 4 + extra Jun 5), 2020 VE Day (Early May BH moved to May 8), 2022 Platinum Jubilee (Spring BH moved to Jun 2 + extra Jun 3), 2022-09-19 State Funeral of Queen Elizabeth II, 2023-05-08 Coronation of King Charles III.

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=next_monday |
| Christmas Day | 12-25; observed=mon_if_sat_tue_if_sun |
| Boxing Day | 12-26; observed=mon_if_sat_tue_if_sun |
| Good Friday | Easter Monday -3 calendar days |
| Easter Monday | Easter Monday +0 calendar days |
| Early May Bank Holiday | 1 occurrence of monday in may; effective [unbounded, 2019] |
| Early May Bank Holiday | 1 occurrence of monday in may; effective [2021, unbounded] |
| Early May Bank Holiday (moved for VE Day 75th anniversary) | 05-08; observed=none; effective [2020, 2020] |
| Spring Bank Holiday | -1 occurrence of monday in may; effective [unbounded, 2011] |
| Spring Bank Holiday | -1 occurrence of monday in may; effective [2013, 2021] |
| Spring Bank Holiday | -1 occurrence of monday in may; effective [2023, unbounded] |
| Spring Bank Holiday (moved for Diamond Jubilee) | 06-04; observed=none; effective [2012, 2012] |
| Queen's Diamond Jubilee | 06-05; observed=none; effective [2012, 2012] |
| Spring Bank Holiday (moved for Platinum Jubilee) | 06-02; observed=none; effective [2022, 2022] |
| Queen's Platinum Jubilee | 06-03; observed=none; effective [2022, 2022] |
| State Funeral of Queen Elizabeth II | 09-19; observed=none; effective [2022, 2022] |
| Coronation of King Charles III | 05-08; observed=none; effective [2023, 2023] |
| Summer Bank Holiday | -1 occurrence of monday in august |

## hkex: Hong Kong Stock Exchange (HKEX) Holidays

Source: [hkex.json](../../../../finstack-quant/core/data/calendars/hkex.json); `ignore_weekends=false`; 1 rules.

| Holiday / selection | Implemented parameters |
|---|---|
| General holidays and jurisdiction-specific substitutions | hong_kong_public_holidays |

## hkhk: Hong Kong (Government) Holiday Calendar

Source: [hkhk.json](../../../../finstack-quant/core/data/calendars/hkhk.json); `ignore_weekends=false`; 1 rules.

| Holiday / selection | Implemented parameters |
|---|---|
| General holidays and jurisdiction-specific substitutions | hong_kong_public_holidays |

## jpto: Japan (Tokyo) Holiday Calendar

Source: [jpto.json](../../../../finstack-quant/core/data/calendars/jpto.json); `ignore_weekends=false`; 4 rules.

| Holiday / selection | Implemented parameters |
|---|---|
| National holidays, Sunday substitutes and citizens' holidays | japan_public_holidays |
| New Year Holiday | 01-02; observed=none |
| New Year Holiday | 01-03; observed=none |
| Bank and Exchange Holiday | 12-31; observed=none |

## jpx: Japan Exchange Group (JPX) Holidays

Source: [jpx.json](../../../../finstack-quant/core/data/calendars/jpx.json); `ignore_weekends=false`; 4 rules.

| Holiday / selection | Implemented parameters |
|---|---|
| National holidays, Sunday substitutes and citizens' holidays | japan_public_holidays |
| New Year Holiday | 01-02; observed=none |
| New Year Holiday | 01-03; observed=none |
| Bank and Exchange Holiday | 12-31; observed=none |

## nerc: NERC (North American Electric Reliability Corporation) Holidays

Source: [nerc.json](../../../../finstack-quant/core/data/calendars/nerc.json); `ignore_weekends=false`; 6 rules.

Data-owner scope/provenance note (not externally revalidated): NERC holidays: the six additional off-peak days of North American power markets. A holiday falling on Sunday is observed the following Monday; a holiday falling on Saturday is not moved. Source: NERC Operating Committee, 'Additional Off-Peak Days' (NERC holiday schedule), as adopted for wholesale power peak/off-peak definitions in NAESB WEQ business practice standards.

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=mon_if_sun |
| Memorial Day | -1 occurrence of monday in may |
| Independence Day | 07-04; observed=mon_if_sun |
| Labor Day | 1 occurrence of monday in september |
| Thanksgiving Day | 4 occurrence of thursday in november |
| Christmas Day | 12-25; observed=mon_if_sun |

## nse: National Stock Exchange of India (NSE) Holiday Calendar

Source: [nse.json](../../../../finstack-quant/core/data/calendars/nse.json); `ignore_weekends=false`; 44 rules.

Data-owner scope/provenance note (not externally revalidated): Recurring Gregorian/statutory rules apply every year. Moving festivals use published NSE circular dates (exact_date) for 2024-2026 only (2024: NSE/CMTR/59722; 2025: NSE/CMTR/65587; 2026: NSE/CMTR/71775). Years outside that verified window have only the statutory/Gregorian rules. Full-day model: Diwali Laxmi Pujan regular sessions are holidays; Muhurat is not modeled. Weekend statutory dates are not observed on an adjacent weekday.

| Holiday / selection | Implemented parameters |
|---|---|
| Republic Day | 01-26; observed=none |
| Maharashtra Day | 05-01; observed=none |
| Independence Day | 08-15; observed=none |
| Gandhi Jayanti | 10-02; observed=none |
| Christmas | 12-25; observed=none |
| Good Friday | Easter Monday -3 calendar days |
| Mahashivratri 2024 | 2024-03-08 |
| Holi 2024 | 2024-03-25 |
| Id-Ul-Fitr 2024 | 2024-04-11 |
| Ambedkar Jayanti 2024 | 2024-04-14 |
| Ram Navami 2024 | 2024-04-17 |
| Mahavir Jayanti 2024 | 2024-04-21 |
| Bakri Id 2024 | 2024-06-17 |
| Muharram 2024 | 2024-07-17 |
| Ganesh Chaturthi 2024 | 2024-09-07 |
| Dussehra 2024 | 2024-10-12 |
| Diwali Laxmi Pujan 2024 | 2024-11-01 |
| Diwali-Balipratipada 2024 | 2024-11-02 |
| Guru Nanak Jayanti 2024 | 2024-11-15 |
| Mahashivratri 2025 | 2025-02-26 |
| Holi 2025 | 2025-03-14 |
| Id-Ul-Fitr 2025 | 2025-03-31 |
| Ram Navami 2025 | 2025-04-06 |
| Mahavir Jayanti 2025 | 2025-04-10 |
| Ambedkar Jayanti 2025 | 2025-04-14 |
| Bakri Id 2025 | 2025-06-07 |
| Muharram 2025 | 2025-07-06 |
| Ganesh Chaturthi 2025 | 2025-08-27 |
| Diwali Laxmi Pujan 2025 | 2025-10-21 |
| Diwali-Balipratipada 2025 | 2025-10-22 |
| Guru Nanak Jayanti 2025 | 2025-11-05 |
| Mahashivratri 2026 | 2026-02-15 |
| Holi 2026 | 2026-03-03 |
| Id-Ul-Fitr 2026 | 2026-03-21 |
| Ram Navami 2026 | 2026-03-26 |
| Mahavir Jayanti 2026 | 2026-03-31 |
| Ambedkar Jayanti 2026 | 2026-04-14 |
| Bakri Id 2026 | 2026-05-28 |
| Muharram 2026 | 2026-06-26 |
| Ganesh Chaturthi 2026 | 2026-09-14 |
| Dussehra 2026 | 2026-10-20 |
| Diwali Laxmi Pujan 2026 | 2026-11-08 |
| Diwali-Balipratipada 2026 | 2026-11-10 |
| Guru Nanak Jayanti 2026 | 2026-11-24 |

## nyse: New York Stock Exchange (NYSE) Holidays

Source: [nyse.json](../../../../finstack-quant/core/data/calendars/nyse.json); `ignore_weekends=false`; 11 rules.

Data-owner scope/provenance note (not externally revalidated): NYSE cash equity calendar. Saturday New Year has no preceding-Friday closure; other fixed holidays retain nearest-weekday observance. Includes the announced 2025-01-09 mourning closure. Annual fixtures cover 2025-2028; other historical exceptional closures are not exhaustively recorded. <https://www.nyse.com/trade/hours-calendars>

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=mon_if_sun |
| Juneteenth National Independence Day | 06-19; observed=fri_if_sat_mon_if_sun; effective [2022, unbounded] |
| Independence Day | 07-04; observed=fri_if_sat_mon_if_sun |
| Christmas Day | 12-25; observed=fri_if_sat_mon_if_sun |
| Martin Luther King Jr. Day | 3 occurrence of monday in january; effective [1998, unbounded] |
| Washington's Birthday | 3 occurrence of monday in february |
| Memorial Day | -1 occurrence of monday in may |
| Labor Day | 1 occurrence of monday in september |
| Thanksgiving Day | 4 occurrence of thursday in november |
| Good Friday | Easter Monday -3 calendar days |
| National Day of Mourning for President Jimmy Carter | 2025-01-09 |

## nzau: New Zealand (Auckland) Holiday Calendar

Source: [nzau.json](../../../../finstack-quant/core/data/calendars/nzau.json); `ignore_weekends=false`; 45 rules.

Data-owner scope/provenance note (not externally revalidated): Auckland regional public holidays, including Auckland Anniversary Day; not a Wellington or NZX-specific calendar. Matariki dates are legislated only for 2022-2052 (<https://www.legislation.govt.nz/act/public/2022/14/en/latest/>). After 2052 this calendar is incomplete pending further dates. Waitangi/ANZAC weekend Mondayization starts in 2014. Annual published fixtures cover 2026-2027: <https://www.govt.nz/browse/work-and-tax/public-holidays-and-work/dates-for-public-holidays-and-anniversary-days/>

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=mon_if_sat_tue_if_sun |
| Day after New Year's Day | 01-02; observed=mon_if_sat_tue_if_sun |
| Auckland Anniversary Day | monday on or after 01-26 |
| Waitangi Day | 02-06; observed=next_monday; effective [2014, unbounded] |
| Good Friday | Easter Monday -3 calendar days |
| Easter Monday | Easter Monday +0 calendar days |
| ANZAC Day | 04-25; observed=next_monday; effective [2014, unbounded] |
| King's Birthday | 1 occurrence of monday in june |
| Labour Day | 4 occurrence of monday in october |
| Christmas Day | 12-25; observed=mon_if_sat_tue_if_sun |
| Boxing Day | 12-26; observed=mon_if_sat_tue_if_sun |
| Waitangi Day | 02-06; observed=none; effective [unbounded, 2013] |
| ANZAC Day | 04-25; observed=none; effective [unbounded, 2013] |
| Matariki | 2022-06-24 |
| Matariki | 2023-07-14 |
| Matariki | 2024-06-28 |
| Matariki | 2025-06-20 |
| Matariki | 2026-07-10 |
| Matariki | 2027-06-25 |
| Matariki | 2028-07-14 |
| Matariki | 2029-07-06 |
| Matariki | 2030-06-21 |
| Matariki | 2031-07-11 |
| Matariki | 2032-07-02 |
| Matariki | 2033-06-24 |
| Matariki | 2034-07-07 |
| Matariki | 2035-06-29 |
| Matariki | 2036-07-18 |
| Matariki | 2037-07-10 |
| Matariki | 2038-06-25 |
| Matariki | 2039-07-15 |
| Matariki | 2040-07-06 |
| Matariki | 2041-07-19 |
| Matariki | 2042-07-11 |
| Matariki | 2043-07-03 |
| Matariki | 2044-06-24 |
| Matariki | 2045-07-07 |
| Matariki | 2046-06-29 |
| Matariki | 2047-07-19 |
| Matariki | 2048-07-03 |
| Matariki | 2049-06-25 |
| Matariki | 2050-07-15 |
| Matariki | 2051-06-30 |
| Matariki | 2052-06-21 |
| Queen Elizabeth II Memorial Day | 2022-09-26 |

## sgsi: Singapore Holiday Calendar

Source: [sgsi.json](../../../../finstack-quant/core/data/calendars/sgsi.json); `ignore_weekends=false`; 25 rules.

Data-owner scope/provenance note (not externally revalidated): Singapore public holidays with Sunday-only statutory substitution (Saturday does not automatically close Monday). Moving religious holidays and Chinese New Year substitutes are complete only for 2024-2027, using Ministry of Manpower annual announcements; outside those years the recurring rule result is incomplete. Sources: <https://www.mom.gov.sg/newsroom/press-releases/2023/0524-public-holidays-for-2024> ; <https://www.mom.gov.sg/employment-practices/public-holidays> ; <https://www.mom.gov.sg/newsroom/press-releases/2026/0618-public-holidays-for-2027>

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=mon_if_sun |
| Labour Day | 05-01; observed=mon_if_sun |
| National Day | 08-09; observed=mon_if_sun |
| Christmas Day | 12-25; observed=mon_if_sun |
| Chinese New Year | 2 consecutive days from (chinese_new_year), offset +0 |
| Good Friday | Easter Monday -3 calendar days |
| Chinese New Year substitute | 2024-02-12 |
| Hari Raya Puasa | 2024-04-10 |
| Vesak Day | 2024-05-22 |
| Hari Raya Haji | 2024-06-17 |
| Deepavali | 2024-10-31 |
| Hari Raya Puasa | 2025-03-31 |
| Polling Day | 2025-05-03 |
| Vesak Day | 2025-05-12 |
| Hari Raya Haji | 2025-06-07 |
| Deepavali | 2025-10-20 |
| Hari Raya Puasa | 2026-03-21 |
| Hari Raya Haji | 2026-05-27 |
| Vesak Day substitute | 2026-06-01 |
| Deepavali substitute | 2026-11-09 |
| Chinese New Year substitute | 2027-02-08 |
| Hari Raya Puasa | 2027-03-10 |
| Hari Raya Haji | 2027-05-17 |
| Vesak Day | 2027-05-20 |
| Deepavali | 2027-10-28 |

## sifma: SIFMA U.S. Bond Market Holidays

Source: [sifma.json](../../../../finstack-quant/core/data/calendars/sifma.json); `ignore_weekends=false`; 13 rules.

Data-owner scope/provenance note (not externally revalidated): U.S. bond-market full-day trading closures; early closes remain business days. Good Friday 2026 (April 3) is a noon early-close session, not a full closure. Published annual fixture: 2026. Other year-specific exceptions require published-schedule maintenance. This is not the SOFR publication calendar or the exact MBS settlement-date table. <https://www.sifma.org/resources/guides-playbook/holiday-schedule>

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=fri_if_sat_mon_if_sun |
| Juneteenth National Independence Day | 06-19; observed=fri_if_sat_mon_if_sun; effective [2022, unbounded] |
| Independence Day | 07-04; observed=fri_if_sat_mon_if_sun |
| Christmas Day | 12-25; observed=fri_if_sat_mon_if_sun |
| Martin Luther King Jr. Day | 3 occurrence of monday in january; effective [1998, unbounded] |
| Washington's Birthday | 3 occurrence of monday in february |
| Memorial Day | -1 occurrence of monday in may |
| Labor Day | 1 occurrence of monday in september |
| Thanksgiving Day | 4 occurrence of thursday in november |
| Good Friday | Easter Monday -3 calendar days; effective [unbounded, 2025] |
| Good Friday | Easter Monday -3 calendar days; effective [2027, unbounded] |
| Columbus Day | 2 occurrence of monday in october; effective [1971, unbounded] |
| Veterans Day | 11-11; observed=fri_if_sat_mon_if_sun |

## six: SIX Swiss Exchange Holiday Calendar

Source: [six.json](../../../../finstack-quant/core/data/calendars/six.json); `ignore_weekends=false`; 12 rules.

Data-owner scope/provenance note (not externally revalidated): Same recurring Swiss set as chzh, plus Christmas Eve and New Year's Eve. SIX trading calendars 2025/2026 list both as full market closes (not early closes). Early-close days stay business days under the full-day model.

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=none |
| Berchtoldstag | 01-02; observed=none |
| Good Friday | Easter Monday -3 calendar days |
| Easter Monday | Easter Monday +0 calendar days |
| Labour Day | 05-01; observed=none |
| Ascension Day | Easter Monday +38 calendar days |
| Whit Monday | Easter Monday +49 calendar days |
| Swiss National Day | 08-01; observed=none |
| Christmas Eve | 12-24; observed=none |
| Christmas Day | 12-25; observed=none |
| St. Stephen's Day | 12-26; observed=none |
| New Year's Eve | 12-31; observed=none |

## sofr: USD SOFR Fixing and Value-Date Calendar

Source: [sofr.json](../../../../finstack-quant/core/data/calendars/sofr.json); `ignore_weekends=false`; 13 rules.

Data-owner scope/provenance note (not externally revalidated): Treasury repo reference-rate calendar, distinct from Federal Reserve banking days (usny) and bond trading sessions (sifma). Full repo-market closures have no SOFR on or for that date; the preceding business-day fixing carries across the closure. Good Friday remains a closure even when government securities trade an early-close session, as confirmed for 2021, 2023 and 2026. New Year's Day and Veterans Day have no Friday substitute for Saturday holidays; Independence Day, Christmas and Juneteenth (from 2022) do. Sources: <https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates>, <https://www.newyorkfed.org/markets/opolicy/operating_policy_250328a>, <https://www.newyorkfed.org/markets/opolicy/operating_policy_260312a>, <https://www.newyorkfed.org/markets/opolicy/operating_policy_260618a>, <https://www.newyorkfed.org/markets/opolicy/operating_policy_181204>. SOFR first value date was 2018-04-02. Recurring rules project the published convention; future exceptional closures still require announced-schedule maintenance.

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=mon_if_sun |
| Martin Luther King Jr. Day | 3 occurrence of monday in january |
| Washington's Birthday | 3 occurrence of monday in february |
| Good Friday | Easter Monday -3 calendar days |
| Memorial Day | -1 occurrence of monday in may |
| Juneteenth National Independence Day | 06-19; observed=fri_if_sat_mon_if_sun; effective [2022, unbounded] |
| Independence Day | 07-04; observed=fri_if_sat_mon_if_sun |
| Labor Day | 1 occurrence of monday in september |
| Columbus Day | 2 occurrence of monday in october |
| Veterans Day | 11-11; observed=mon_if_sun |
| Thanksgiving Day | 4 occurrence of thursday in november |
| Christmas Day | 12-25; observed=fri_if_sat_mon_if_sun |
| National Day of Mourning for George H. W. Bush | 2018-12-05 |

## sse: Shanghai Stock Exchange (SSE) Holidays

Source: [sse.json](../../../../finstack-quant/core/data/calendars/sse.json); `ignore_weekends=false`; 50 rules.

Data-owner scope/provenance note (not externally revalidated): Mainland China exchange calendar. Spring Festival and Labour Day use published year-specific closure spans for 2004-2026; rules outside that verified range are explicitly estimates. Single-day statutory festivals use the modern 3-day 连休 (china_bridge) convention codified by the State Council (Nov 2024, effective 2025).

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | China bridge around (01-01; observed=none) |
| Estimated Spring Festival (pre-verified coverage) | 7 consecutive days from (chinese_new_year), offset +0; effective [unbounded, 2003] |
| Spring Festival 2004 | 7 consecutive days from (01-22; observed=none), offset +0; effective [2004, 2004] |
| Spring Festival 2005 | 7 consecutive days from (02-09; observed=none), offset +0; effective [2005, 2005] |
| Spring Festival 2006 | 7 consecutive days from (01-29; observed=none), offset +0; effective [2006, 2006] |
| Spring Festival 2007 | 7 consecutive days from (02-18; observed=none), offset +0; effective [2007, 2007] |
| Spring Festival 2008 | 7 consecutive days from (02-06; observed=none), offset +0; effective [2008, 2008] |
| Spring Festival 2009 | 7 consecutive days from (01-25; observed=none), offset +0; effective [2009, 2009] |
| Spring Festival 2010 | 7 consecutive days from (02-13; observed=none), offset +0; effective [2010, 2010] |
| Spring Festival 2011 | 7 consecutive days from (02-02; observed=none), offset +0; effective [2011, 2011] |
| Spring Festival 2012 | 7 consecutive days from (01-22; observed=none), offset +0; effective [2012, 2012] |
| Spring Festival 2013 | 7 consecutive days from (02-09; observed=none), offset +0; effective [2013, 2013] |
| Spring Festival 2014 | 7 consecutive days from (01-31; observed=none), offset +0; effective [2014, 2014] |
| Spring Festival 2015 | 7 consecutive days from (02-18; observed=none), offset +0; effective [2015, 2015] |
| Spring Festival 2016 | 7 consecutive days from (02-07; observed=none), offset +0; effective [2016, 2016] |
| Spring Festival 2017 | 7 consecutive days from (01-27; observed=none), offset +0; effective [2017, 2017] |
| Spring Festival 2018 | 7 consecutive days from (02-15; observed=none), offset +0; effective [2018, 2018] |
| Spring Festival 2019 | 7 consecutive days from (02-04; observed=none), offset +0; effective [2019, 2019] |
| Spring Festival 2020 extended closure | 10 consecutive days from (01-24; observed=none), offset +0; effective [2020, 2020] |
| Spring Festival 2021 | 7 consecutive days from (02-11; observed=none), offset +0; effective [2021, 2021] |
| Spring Festival 2022 | 7 consecutive days from (01-31; observed=none), offset +0; effective [2022, 2022] |
| Spring Festival 2023 | 7 consecutive days from (01-21; observed=none), offset +0; effective [2023, 2023] |
| Spring Festival 2024 | 9 consecutive days from (02-09; observed=none), offset +0; effective [2024, 2024] |
| Spring Festival 2025 | 8 consecutive days from (01-28; observed=none), offset +0; effective [2025, 2025] |
| Spring Festival 2026 | 9 consecutive days from (02-15; observed=none), offset +0; effective [2026, 2026] |
| Estimated Spring Festival (post-verified coverage) | 8 consecutive days from (chinese_new_year), offset -1; effective [2027, unbounded] |
| Qingming Festival | China bridge around (qing_ming); effective [2008, unbounded] |
| Estimated Labour Day Golden Week (pre-verified coverage) | 7 consecutive days from (05-01; observed=none), offset +0; effective [unbounded, 2003] |
| Labour Day Golden Week 2004-2007 | 7 consecutive days from (05-01; observed=none), offset +0; effective [2004, 2007] |
| Labour Day 2008-2010 | 3 consecutive days from (05-01; observed=none), offset +0; effective [2008, 2010] |
| Labour Day 2011 | 3 consecutive days from (04-30; observed=none), offset +0; effective [2011, 2011] |
| Labour Day 2012 | 3 consecutive days from (04-29; observed=none), offset +0; effective [2012, 2012] |
| Labour Day 2013 | 3 consecutive days from (04-29; observed=none), offset +0; effective [2013, 2013] |
| Labour Day 2014-2015 | 3 consecutive days from (05-01; observed=none), offset +0; effective [2014, 2015] |
| Labour Day 2016 | 3 consecutive days from (04-30; observed=none), offset +0; effective [2016, 2016] |
| Labour Day 2017 | 3 consecutive days from (04-29; observed=none), offset +0; effective [2017, 2017] |
| Labour Day 2018 | 3 consecutive days from (04-29; observed=none), offset +0; effective [2018, 2018] |
| Labour Day 2019 | 4 consecutive days from (05-01; observed=none), offset +0; effective [2019, 2019] |
| Labour Day 2020-2021 | 5 consecutive days from (05-01; observed=none), offset +0; effective [2020, 2021] |
| Labour Day 2022 | 5 consecutive days from (04-30; observed=none), offset +0; effective [2022, 2022] |
| Labour Day 2023 | 5 consecutive days from (04-29; observed=none), offset +0; effective [2023, 2023] |
| Labour Day 2024-2026 | 5 consecutive days from (05-01; observed=none), offset +0; effective [2024, 2026] |
| Estimated Labour Day (post-verified coverage) | 5 consecutive days from (05-01; observed=none), offset +0; effective [2027, unbounded] |
| Dragon Boat Festival | China bridge around (dragon_boat); effective [2008, unbounded] |
| Mid-Autumn Festival | China bridge around (mid_autumn); effective [2008, unbounded] |
| National Day Golden Week | 7 consecutive days from (10-01; observed=none), offset +0 |
| National Day Golden Week 2009 extended closure | 2009-10-08 |
| Mid-Autumn Festival 2010 published closure tail | 2 consecutive days from (09-23; observed=none), offset +0; effective [2010, 2010] |
| 70th anniversary commemoration closure | 2 consecutive days from (09-03; observed=none), offset +0; effective [2015, 2015] |
| National Day / Mid-Autumn merged 8th day (2025) | 2025-10-08 |

## target2: ECB TARGET2 Settlement Holidays

Source: [target2.json](../../../../finstack-quant/core/data/calendars/target2.json); `ignore_weekends=false`; 6 rules.

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=none |
| Good Friday | Easter Monday -3 calendar days |
| Easter Monday | Easter Monday +0 calendar days |
| Labour Day | 05-01; observed=none |
| Christmas Day | 12-25; observed=none |
| St. Stephen's Day | 12-26; observed=none |

## tsx: Toronto Stock Exchange (TSX) Holiday Calendar

Source: [tsx.json](../../../../finstack-quant/core/data/calendars/tsx.json); `ignore_weekends=false`; 10 rules.

Data-owner scope/provenance note (not externally revalidated): TSX trading calendar excludes Remembrance Day and National Day for Truth and Reconciliation. Victoria Day is Monday on or before May 24. Christmas and Boxing Day have distinct substitute dates. Family Day applies from 2008. Published annual fixture: 2026. <https://www.tsx.com/en/trading/calendars-and-trading-hours/calendar>

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=next_monday |
| Canada Day | 07-01; observed=next_monday |
| Christmas Day | 12-25; observed=mon_if_sat_tue_if_sun |
| Boxing Day | 12-26; observed=mon_if_sat_tue_if_sun |
| Family Day | 3 occurrence of monday in february; effective [2008, unbounded] |
| Good Friday | Easter Monday -3 calendar days |
| Victoria Day | monday on or before 05-24 |
| Civic Holiday | 1 occurrence of monday in august |
| Labour Day | 1 occurrence of monday in september |
| Thanksgiving | 2 occurrence of monday in october |

## usny: United States (New York Federal) Holidays

Source: [usny.json](../../../../finstack-quant/core/data/calendars/usny.json); `ignore_weekends=false`; 11 rules.

Data-owner scope/provenance note (not externally revalidated): Federal Reserve observance convention (basis for Fedwire business days): a holiday falling on Sunday is observed the following Monday; a holiday falling on Saturday has NO substitute (Federal Reserve Banks are open the preceding Friday). Source: Federal Reserve Board, 'K.8 Holidays Observed by the Federal Reserve System'. This differs from the OPM/NYSE rule (fri_if_sat_mon_if_sun) used by nyse.json and sifma.json.

| Holiday / selection | Implemented parameters |
|---|---|
| New Year's Day | 01-01; observed=mon_if_sun |
| Martin Luther King Jr. Day | 3 occurrence of monday in january; effective [1986, unbounded] |
| Washington's Birthday | 3 occurrence of monday in february |
| Memorial Day | -1 occurrence of monday in may |
| Juneteenth National Independence Day | 06-19; observed=mon_if_sun; effective [2021, unbounded] |
| Independence Day | 07-04; observed=mon_if_sun |
| Labor Day | 1 occurrence of monday in september |
| Columbus Day | 2 occurrence of monday in october |
| Veterans Day | 11-11; observed=mon_if_sun |
| Thanksgiving Day | 4 occurrence of thursday in november |
| Christmas Day | 12-25; observed=mon_if_sun |

## weekends_only: Weekends Only

Source: [weekends_only.json](../../../../finstack-quant/core/data/calendars/weekends_only.json); `ignore_weekends=true`; 0 rules.

No named holidays; the business-day predicate excludes weekends.
