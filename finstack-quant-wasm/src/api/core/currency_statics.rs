//! `Currency` static factories, one per ISO-4217 code in
//! `finstack-quant/core/data/iso_4217.csv` (the source of the Rust `Currency`
//! enum). `Currency.usd()` is the twin of the Python module constant
//! `finstack_quant.core.currency.USD`.
//!
//! Each entry carries its own documentation, which becomes the published
//! JSDoc; the test below fails when the list drifts from `Currency::iter()`.

use crate::api::core::currency::JsCurrency;
use finstack_quant_core::currency::Currency as RustCurrency;
use wasm_bindgen::prelude::*;

/// One zero-argument static factory per `name as "jsName" => VARIANT` entry.
macro_rules! currency_factories {
    ($( $(#[$doc:meta])* $name:ident as $js_name:literal => $variant:ident ),* $(,)?) => {
        #[wasm_bindgen(js_class = Currency)]
        impl JsCurrency {
            $(
                $(#[$doc])*
                #[wasm_bindgen(js_name = $js_name)]
                pub fn $name() -> JsCurrency {
                    JsCurrency {
                        inner: RustCurrency::$variant,
                    }
                }
            )*
        }

        /// Codes with a static factory, in declaration order.
        #[cfg(test)]
        const FACTORY_CODES: &[&str] = &[$(stringify!($variant)),*];
    };
}

currency_factories! {
    /// UAE Dirham (`AED`, ISO-4217 numeric 784).
    ///
    /// @returns The `AED` currency.
    aed as "aed" => AED,
    /// Afghani (`AFN`, ISO-4217 numeric 971).
    ///
    /// @returns The `AFN` currency.
    afn as "afn" => AFN,
    /// Lek (`ALL`, ISO-4217 numeric 8).
    ///
    /// @returns The `ALL` currency.
    all as "all" => ALL,
    /// Armenian Dram (`AMD`, ISO-4217 numeric 51).
    ///
    /// @returns The `AMD` currency.
    amd as "amd" => AMD,
    /// Netherlands Antillean Guilder (`ANG`, ISO-4217 numeric 532).
    ///
    /// @returns The `ANG` currency.
    ang as "ang" => ANG,
    /// Kwanza (`AOA`, ISO-4217 numeric 973).
    ///
    /// @returns The `AOA` currency.
    aoa as "aoa" => AOA,
    /// Argentine Peso (`ARS`, ISO-4217 numeric 32).
    ///
    /// @returns The `ARS` currency.
    ars as "ars" => ARS,
    /// Australian Dollar (`AUD`, ISO-4217 numeric 36).
    ///
    /// @returns The `AUD` currency.
    aud as "aud" => AUD,
    /// Aruban Florin (`AWG`, ISO-4217 numeric 533).
    ///
    /// @returns The `AWG` currency.
    awg as "awg" => AWG,
    /// Azerbaijan Manat (`AZN`, ISO-4217 numeric 944).
    ///
    /// @returns The `AZN` currency.
    azn as "azn" => AZN,
    /// Convertible Mark (`BAM`, ISO-4217 numeric 977).
    ///
    /// @returns The `BAM` currency.
    bam as "bam" => BAM,
    /// Barbados Dollar (`BBD`, ISO-4217 numeric 52).
    ///
    /// @returns The `BBD` currency.
    bbd as "bbd" => BBD,
    /// Taka (`BDT`, ISO-4217 numeric 50).
    ///
    /// @returns The `BDT` currency.
    bdt as "bdt" => BDT,
    /// Bulgarian Lev (`BGN`, ISO-4217 numeric 975).
    ///
    /// @returns The `BGN` currency.
    bgn as "bgn" => BGN,
    /// Bahraini Dinar (`BHD`, ISO-4217 numeric 48).
    ///
    /// @returns The `BHD` currency.
    bhd as "bhd" => BHD,
    /// Burundi Franc (`BIF`, ISO-4217 numeric 108).
    ///
    /// @returns The `BIF` currency.
    bif as "bif" => BIF,
    /// Bermudian Dollar (`BMD`, ISO-4217 numeric 60).
    ///
    /// @returns The `BMD` currency.
    bmd as "bmd" => BMD,
    /// Brunei Dollar (`BND`, ISO-4217 numeric 96).
    ///
    /// @returns The `BND` currency.
    bnd as "bnd" => BND,
    /// Boliviano (`BOB`, ISO-4217 numeric 68).
    ///
    /// @returns The `BOB` currency.
    bob as "bob" => BOB,
    /// Brazilian Real (`BRL`, ISO-4217 numeric 986).
    ///
    /// @returns The `BRL` currency.
    brl as "brl" => BRL,
    /// Bahamian Dollar (`BSD`, ISO-4217 numeric 44).
    ///
    /// @returns The `BSD` currency.
    bsd as "bsd" => BSD,
    /// Ngultrum (`BTN`, ISO-4217 numeric 64).
    ///
    /// @returns The `BTN` currency.
    btn as "btn" => BTN,
    /// Pula (`BWP`, ISO-4217 numeric 72).
    ///
    /// @returns The `BWP` currency.
    bwp as "bwp" => BWP,
    /// Belarusian Ruble (`BYN`, ISO-4217 numeric 933).
    ///
    /// @returns The `BYN` currency.
    byn as "byn" => BYN,
    /// Belize Dollar (`BZD`, ISO-4217 numeric 84).
    ///
    /// @returns The `BZD` currency.
    bzd as "bzd" => BZD,
    /// Canadian Dollar (`CAD`, ISO-4217 numeric 124).
    ///
    /// @returns The `CAD` currency.
    cad as "cad" => CAD,
    /// Congolese Franc (`CDF`, ISO-4217 numeric 976).
    ///
    /// @returns The `CDF` currency.
    cdf as "cdf" => CDF,
    /// Swiss Franc (`CHF`, ISO-4217 numeric 756).
    ///
    /// @returns The `CHF` currency.
    chf as "chf" => CHF,
    /// Unidad de Fomento (`CLF`, ISO-4217 numeric 990).
    ///
    /// @returns The `CLF` currency.
    clf as "clf" => CLF,
    /// Chilean Peso (`CLP`, ISO-4217 numeric 152).
    ///
    /// @returns The `CLP` currency.
    clp as "clp" => CLP,
    /// Yuan Renminbi (`CNY`, ISO-4217 numeric 156).
    ///
    /// @returns The `CNY` currency.
    cny as "cny" => CNY,
    /// Colombian Peso (`COP`, ISO-4217 numeric 170).
    ///
    /// @returns The `COP` currency.
    cop as "cop" => COP,
    /// Costa Rican Colon (`CRC`, ISO-4217 numeric 188).
    ///
    /// @returns The `CRC` currency.
    crc as "crc" => CRC,
    /// Peso Convertible (`CUC`, ISO-4217 numeric 931).
    ///
    /// @returns The `CUC` currency.
    cuc as "cuc" => CUC,
    /// Cuban Peso (`CUP`, ISO-4217 numeric 192).
    ///
    /// @returns The `CUP` currency.
    cup as "cup" => CUP,
    /// Cabo Verde Escudo (`CVE`, ISO-4217 numeric 132).
    ///
    /// @returns The `CVE` currency.
    cve as "cve" => CVE,
    /// Czech Koruna (`CZK`, ISO-4217 numeric 203).
    ///
    /// @returns The `CZK` currency.
    czk as "czk" => CZK,
    /// Djibouti Franc (`DJF`, ISO-4217 numeric 262).
    ///
    /// @returns The `DJF` currency.
    djf as "djf" => DJF,
    /// Danish Krone (`DKK`, ISO-4217 numeric 208).
    ///
    /// @returns The `DKK` currency.
    dkk as "dkk" => DKK,
    /// Dominican Peso (`DOP`, ISO-4217 numeric 214).
    ///
    /// @returns The `DOP` currency.
    dop as "dop" => DOP,
    /// Algerian Dinar (`DZD`, ISO-4217 numeric 12).
    ///
    /// @returns The `DZD` currency.
    dzd as "dzd" => DZD,
    /// Egyptian Pound (`EGP`, ISO-4217 numeric 818).
    ///
    /// @returns The `EGP` currency.
    egp as "egp" => EGP,
    /// Nakfa (`ERN`, ISO-4217 numeric 232).
    ///
    /// @returns The `ERN` currency.
    ern as "ern" => ERN,
    /// Ethiopian Birr (`ETB`, ISO-4217 numeric 230).
    ///
    /// @returns The `ETB` currency.
    etb as "etb" => ETB,
    /// Euro (`EUR`, ISO-4217 numeric 978).
    ///
    /// @returns The `EUR` currency.
    eur as "eur" => EUR,
    /// Fiji Dollar (`FJD`, ISO-4217 numeric 242).
    ///
    /// @returns The `FJD` currency.
    fjd as "fjd" => FJD,
    /// Falkland Islands Pound (`FKP`, ISO-4217 numeric 238).
    ///
    /// @returns The `FKP` currency.
    fkp as "fkp" => FKP,
    /// Pound Sterling (`GBP`, ISO-4217 numeric 826).
    ///
    /// @returns The `GBP` currency.
    gbp as "gbp" => GBP,
    /// Lari (`GEL`, ISO-4217 numeric 981).
    ///
    /// @returns The `GEL` currency.
    gel as "gel" => GEL,
    /// Ghana Cedi (`GHS`, ISO-4217 numeric 936).
    ///
    /// @returns The `GHS` currency.
    ghs as "ghs" => GHS,
    /// Gibraltar Pound (`GIP`, ISO-4217 numeric 292).
    ///
    /// @returns The `GIP` currency.
    gip as "gip" => GIP,
    /// Dalasi (`GMD`, ISO-4217 numeric 270).
    ///
    /// @returns The `GMD` currency.
    gmd as "gmd" => GMD,
    /// Guinean Franc (`GNF`, ISO-4217 numeric 324).
    ///
    /// @returns The `GNF` currency.
    gnf as "gnf" => GNF,
    /// Quetzal (`GTQ`, ISO-4217 numeric 320).
    ///
    /// @returns The `GTQ` currency.
    gtq as "gtq" => GTQ,
    /// Guyana Dollar (`GYD`, ISO-4217 numeric 328).
    ///
    /// @returns The `GYD` currency.
    gyd as "gyd" => GYD,
    /// Hong Kong Dollar (`HKD`, ISO-4217 numeric 344).
    ///
    /// @returns The `HKD` currency.
    hkd as "hkd" => HKD,
    /// Lempira (`HNL`, ISO-4217 numeric 340).
    ///
    /// @returns The `HNL` currency.
    hnl as "hnl" => HNL,
    /// Kuna (`HRK`, ISO-4217 numeric 191).
    ///
    /// @returns The `HRK` currency.
    hrk as "hrk" => HRK,
    /// Gourde (`HTG`, ISO-4217 numeric 332).
    ///
    /// @returns The `HTG` currency.
    htg as "htg" => HTG,
    /// Forint (`HUF`, ISO-4217 numeric 348).
    ///
    /// @returns The `HUF` currency.
    huf as "huf" => HUF,
    /// Rupiah (`IDR`, ISO-4217 numeric 360).
    ///
    /// @returns The `IDR` currency.
    idr as "idr" => IDR,
    /// New Israeli Sheqel (`ILS`, ISO-4217 numeric 376).
    ///
    /// @returns The `ILS` currency.
    ils as "ils" => ILS,
    /// Indian Rupee (`INR`, ISO-4217 numeric 356).
    ///
    /// @returns The `INR` currency.
    inr as "inr" => INR,
    /// Iraqi Dinar (`IQD`, ISO-4217 numeric 368).
    ///
    /// @returns The `IQD` currency.
    iqd as "iqd" => IQD,
    /// Iranian Rial (`IRR`, ISO-4217 numeric 364).
    ///
    /// @returns The `IRR` currency.
    irr as "irr" => IRR,
    /// Iceland Krona (`ISK`, ISO-4217 numeric 352).
    ///
    /// @returns The `ISK` currency.
    isk as "isk" => ISK,
    /// Jamaican Dollar (`JMD`, ISO-4217 numeric 388).
    ///
    /// @returns The `JMD` currency.
    jmd as "jmd" => JMD,
    /// Jordanian Dinar (`JOD`, ISO-4217 numeric 400).
    ///
    /// @returns The `JOD` currency.
    jod as "jod" => JOD,
    /// Yen (`JPY`, ISO-4217 numeric 392).
    ///
    /// @returns The `JPY` currency.
    jpy as "jpy" => JPY,
    /// Kenyan Shilling (`KES`, ISO-4217 numeric 404).
    ///
    /// @returns The `KES` currency.
    kes as "kes" => KES,
    /// Som (`KGS`, ISO-4217 numeric 417).
    ///
    /// @returns The `KGS` currency.
    kgs as "kgs" => KGS,
    /// Riel (`KHR`, ISO-4217 numeric 116).
    ///
    /// @returns The `KHR` currency.
    khr as "khr" => KHR,
    /// Comorian Franc (`KMF`, ISO-4217 numeric 174).
    ///
    /// @returns The `KMF` currency.
    kmf as "kmf" => KMF,
    /// North Korean Won (`KPW`, ISO-4217 numeric 408).
    ///
    /// @returns The `KPW` currency.
    kpw as "kpw" => KPW,
    /// Won (`KRW`, ISO-4217 numeric 410).
    ///
    /// @returns The `KRW` currency.
    krw as "krw" => KRW,
    /// Kuwaiti Dinar (`KWD`, ISO-4217 numeric 414).
    ///
    /// @returns The `KWD` currency.
    kwd as "kwd" => KWD,
    /// Cayman Islands Dollar (`KYD`, ISO-4217 numeric 136).
    ///
    /// @returns The `KYD` currency.
    kyd as "kyd" => KYD,
    /// Tenge (`KZT`, ISO-4217 numeric 398).
    ///
    /// @returns The `KZT` currency.
    kzt as "kzt" => KZT,
    /// Lao Kip (`LAK`, ISO-4217 numeric 418).
    ///
    /// @returns The `LAK` currency.
    lak as "lak" => LAK,
    /// Lebanese Pound (`LBP`, ISO-4217 numeric 422).
    ///
    /// @returns The `LBP` currency.
    lbp as "lbp" => LBP,
    /// Sri Lanka Rupee (`LKR`, ISO-4217 numeric 144).
    ///
    /// @returns The `LKR` currency.
    lkr as "lkr" => LKR,
    /// Liberian Dollar (`LRD`, ISO-4217 numeric 430).
    ///
    /// @returns The `LRD` currency.
    lrd as "lrd" => LRD,
    /// Loti (`LSL`, ISO-4217 numeric 426).
    ///
    /// @returns The `LSL` currency.
    lsl as "lsl" => LSL,
    /// Libyan Dinar (`LYD`, ISO-4217 numeric 434).
    ///
    /// @returns The `LYD` currency.
    lyd as "lyd" => LYD,
    /// Moroccan Dirham (`MAD`, ISO-4217 numeric 504).
    ///
    /// @returns The `MAD` currency.
    mad as "mad" => MAD,
    /// Moldovan Leu (`MDL`, ISO-4217 numeric 498).
    ///
    /// @returns The `MDL` currency.
    mdl as "mdl" => MDL,
    /// Malagasy Ariary (`MGA`, ISO-4217 numeric 969).
    ///
    /// @returns The `MGA` currency.
    mga as "mga" => MGA,
    /// Denar (`MKD`, ISO-4217 numeric 807).
    ///
    /// @returns The `MKD` currency.
    mkd as "mkd" => MKD,
    /// Kyat (`MMK`, ISO-4217 numeric 104).
    ///
    /// @returns The `MMK` currency.
    mmk as "mmk" => MMK,
    /// Tugrik (`MNT`, ISO-4217 numeric 496).
    ///
    /// @returns The `MNT` currency.
    mnt as "mnt" => MNT,
    /// Pataca (`MOP`, ISO-4217 numeric 446).
    ///
    /// @returns The `MOP` currency.
    mop as "mop" => MOP,
    /// Ouguiya (`MRU`, ISO-4217 numeric 929).
    ///
    /// @returns The `MRU` currency.
    mru as "mru" => MRU,
    /// Mauritius Rupee (`MUR`, ISO-4217 numeric 480).
    ///
    /// @returns The `MUR` currency.
    mur as "mur" => MUR,
    /// Rufiyaa (`MVR`, ISO-4217 numeric 462).
    ///
    /// @returns The `MVR` currency.
    mvr as "mvr" => MVR,
    /// Malawi Kwacha (`MWK`, ISO-4217 numeric 454).
    ///
    /// @returns The `MWK` currency.
    mwk as "mwk" => MWK,
    /// Mexican Peso (`MXN`, ISO-4217 numeric 484).
    ///
    /// @returns The `MXN` currency.
    mxn as "mxn" => MXN,
    /// Malaysian Ringgit (`MYR`, ISO-4217 numeric 458).
    ///
    /// @returns The `MYR` currency.
    myr as "myr" => MYR,
    /// Mozambique Metical (`MZN`, ISO-4217 numeric 943).
    ///
    /// @returns The `MZN` currency.
    mzn as "mzn" => MZN,
    /// Namibia Dollar (`NAD`, ISO-4217 numeric 516).
    ///
    /// @returns The `NAD` currency.
    nad as "nad" => NAD,
    /// Naira (`NGN`, ISO-4217 numeric 566).
    ///
    /// @returns The `NGN` currency.
    ngn as "ngn" => NGN,
    /// Cordoba Oro (`NIO`, ISO-4217 numeric 558).
    ///
    /// @returns The `NIO` currency.
    nio as "nio" => NIO,
    /// Norwegian Krone (`NOK`, ISO-4217 numeric 578).
    ///
    /// @returns The `NOK` currency.
    nok as "nok" => NOK,
    /// Nepalese Rupee (`NPR`, ISO-4217 numeric 524).
    ///
    /// @returns The `NPR` currency.
    npr as "npr" => NPR,
    /// New Zealand Dollar (`NZD`, ISO-4217 numeric 554).
    ///
    /// @returns The `NZD` currency.
    nzd as "nzd" => NZD,
    /// Rial Omani (`OMR`, ISO-4217 numeric 512).
    ///
    /// @returns The `OMR` currency.
    omr as "omr" => OMR,
    /// Balboa (`PAB`, ISO-4217 numeric 590).
    ///
    /// @returns The `PAB` currency.
    pab as "pab" => PAB,
    /// Sol (`PEN`, ISO-4217 numeric 604).
    ///
    /// @returns The `PEN` currency.
    pen as "pen" => PEN,
    /// Kina (`PGK`, ISO-4217 numeric 598).
    ///
    /// @returns The `PGK` currency.
    pgk as "pgk" => PGK,
    /// Philippine Peso (`PHP`, ISO-4217 numeric 608).
    ///
    /// @returns The `PHP` currency.
    php as "php" => PHP,
    /// Pakistan Rupee (`PKR`, ISO-4217 numeric 586).
    ///
    /// @returns The `PKR` currency.
    pkr as "pkr" => PKR,
    /// Zloty (`PLN`, ISO-4217 numeric 985).
    ///
    /// @returns The `PLN` currency.
    pln as "pln" => PLN,
    /// Guarani (`PYG`, ISO-4217 numeric 600).
    ///
    /// @returns The `PYG` currency.
    pyg as "pyg" => PYG,
    /// Qatari Rial (`QAR`, ISO-4217 numeric 634).
    ///
    /// @returns The `QAR` currency.
    qar as "qar" => QAR,
    /// Romanian Leu (`RON`, ISO-4217 numeric 946).
    ///
    /// @returns The `RON` currency.
    ron as "ron" => RON,
    /// Serbian Dinar (`RSD`, ISO-4217 numeric 941).
    ///
    /// @returns The `RSD` currency.
    rsd as "rsd" => RSD,
    /// Russian Ruble (`RUB`, ISO-4217 numeric 643).
    ///
    /// @returns The `RUB` currency.
    rub as "rub" => RUB,
    /// Rwanda Franc (`RWF`, ISO-4217 numeric 646).
    ///
    /// @returns The `RWF` currency.
    rwf as "rwf" => RWF,
    /// Saudi Riyal (`SAR`, ISO-4217 numeric 682).
    ///
    /// @returns The `SAR` currency.
    sar as "sar" => SAR,
    /// Solomon Islands Dollar (`SBD`, ISO-4217 numeric 90).
    ///
    /// @returns The `SBD` currency.
    sbd as "sbd" => SBD,
    /// Seychelles Rupee (`SCR`, ISO-4217 numeric 690).
    ///
    /// @returns The `SCR` currency.
    scr as "scr" => SCR,
    /// Sudanese Pound (`SDG`, ISO-4217 numeric 938).
    ///
    /// @returns The `SDG` currency.
    sdg as "sdg" => SDG,
    /// Swedish Krona (`SEK`, ISO-4217 numeric 752).
    ///
    /// @returns The `SEK` currency.
    sek as "sek" => SEK,
    /// Singapore Dollar (`SGD`, ISO-4217 numeric 702).
    ///
    /// @returns The `SGD` currency.
    sgd as "sgd" => SGD,
    /// Saint Helena Pound (`SHP`, ISO-4217 numeric 654).
    ///
    /// @returns The `SHP` currency.
    shp as "shp" => SHP,
    /// Leone (`SLE`, ISO-4217 numeric 925).
    ///
    /// @returns The `SLE` currency.
    sle as "sle" => SLE,
    /// Leone (`SLL`, ISO-4217 numeric 694).
    ///
    /// @returns The `SLL` currency.
    sll as "sll" => SLL,
    /// Somali Shilling (`SOS`, ISO-4217 numeric 706).
    ///
    /// @returns The `SOS` currency.
    sos as "sos" => SOS,
    /// Surinam Dollar (`SRD`, ISO-4217 numeric 968).
    ///
    /// @returns The `SRD` currency.
    srd as "srd" => SRD,
    /// South Sudanese Pound (`SSP`, ISO-4217 numeric 728).
    ///
    /// @returns The `SSP` currency.
    ssp as "ssp" => SSP,
    /// Dobra (`STN`, ISO-4217 numeric 930).
    ///
    /// @returns The `STN` currency.
    stn as "stn" => STN,
    /// Syrian Pound (`SYP`, ISO-4217 numeric 760).
    ///
    /// @returns The `SYP` currency.
    syp as "syp" => SYP,
    /// Lilangeni (`SZL`, ISO-4217 numeric 748).
    ///
    /// @returns The `SZL` currency.
    szl as "szl" => SZL,
    /// Baht (`THB`, ISO-4217 numeric 764).
    ///
    /// @returns The `THB` currency.
    thb as "thb" => THB,
    /// Somoni (`TJS`, ISO-4217 numeric 972).
    ///
    /// @returns The `TJS` currency.
    tjs as "tjs" => TJS,
    /// Turkmenistan New Manat (`TMT`, ISO-4217 numeric 934).
    ///
    /// @returns The `TMT` currency.
    tmt as "tmt" => TMT,
    /// Tunisian Dinar (`TND`, ISO-4217 numeric 788).
    ///
    /// @returns The `TND` currency.
    tnd as "tnd" => TND,
    /// Pa'anga (`TOP`, ISO-4217 numeric 776).
    ///
    /// @returns The `TOP` currency.
    top as "top" => TOP,
    /// Turkish Lira (`TRY`, ISO-4217 numeric 949).
    ///
    /// @returns The `TRY` currency.
    r#try as "try" => TRY,
    /// Trinidad and Tobago Dollar (`TTD`, ISO-4217 numeric 780).
    ///
    /// @returns The `TTD` currency.
    ttd as "ttd" => TTD,
    /// New Taiwan Dollar (`TWD`, ISO-4217 numeric 901).
    ///
    /// @returns The `TWD` currency.
    twd as "twd" => TWD,
    /// Tanzanian Shilling (`TZS`, ISO-4217 numeric 834).
    ///
    /// @returns The `TZS` currency.
    tzs as "tzs" => TZS,
    /// Hryvnia (`UAH`, ISO-4217 numeric 980).
    ///
    /// @returns The `UAH` currency.
    uah as "uah" => UAH,
    /// Uganda Shilling (`UGX`, ISO-4217 numeric 800).
    ///
    /// @returns The `UGX` currency.
    ugx as "ugx" => UGX,
    /// US Dollar (`USD`, ISO-4217 numeric 840).
    ///
    /// @returns The `USD` currency.
    usd as "usd" => USD,
    /// Peso Uruguayo (`UYU`, ISO-4217 numeric 858).
    ///
    /// @returns The `UYU` currency.
    uyu as "uyu" => UYU,
    /// Uzbekistan Sum (`UZS`, ISO-4217 numeric 860).
    ///
    /// @returns The `UZS` currency.
    uzs as "uzs" => UZS,
    /// Bolívar Soberano (`VED`, ISO-4217 numeric 926).
    ///
    /// @returns The `VED` currency.
    ved as "ved" => VED,
    /// Bolívar Soberano (`VES`, ISO-4217 numeric 928).
    ///
    /// @returns The `VES` currency.
    ves as "ves" => VES,
    /// Dong (`VND`, ISO-4217 numeric 704).
    ///
    /// @returns The `VND` currency.
    vnd as "vnd" => VND,
    /// Vatu (`VUV`, ISO-4217 numeric 548).
    ///
    /// @returns The `VUV` currency.
    vuv as "vuv" => VUV,
    /// Tala (`WST`, ISO-4217 numeric 882).
    ///
    /// @returns The `WST` currency.
    wst as "wst" => WST,
    /// CFA Franc BEAC (`XAF`, ISO-4217 numeric 950).
    ///
    /// @returns The `XAF` currency.
    xaf as "xaf" => XAF,
    /// East Caribbean Dollar (`XCD`, ISO-4217 numeric 951).
    ///
    /// @returns The `XCD` currency.
    xcd as "xcd" => XCD,
    /// CFA Franc BCEAO (`XOF`, ISO-4217 numeric 952).
    ///
    /// @returns The `XOF` currency.
    xof as "xof" => XOF,
    /// CFP Franc (`XPF`, ISO-4217 numeric 953).
    ///
    /// @returns The `XPF` currency.
    xpf as "xpf" => XPF,
    /// Yemeni Rial (`YER`, ISO-4217 numeric 886).
    ///
    /// @returns The `YER` currency.
    yer as "yer" => YER,
    /// Rand (`ZAR`, ISO-4217 numeric 710).
    ///
    /// @returns The `ZAR` currency.
    zar as "zar" => ZAR,
    /// Zambian Kwacha (`ZMW`, ISO-4217 numeric 967).
    ///
    /// @returns The `ZMW` currency.
    zmw as "zmw" => ZMW,
    /// Zimbabwe Dollar (`ZWL`, ISO-4217 numeric 932).
    ///
    /// @returns The `ZWL` currency.
    zwl as "zwl" => ZWL,
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn every_currency_has_a_static_factory() {
        let expected: Vec<String> = RustCurrency::iter().map(|c| c.to_string()).collect();
        assert_eq!(
            FACTORY_CODES, expected,
            "add the missing code to currency_statics.rs from finstack-quant/core/data/iso_4217.csv"
        );
    }

    #[test]
    fn factories_return_their_own_code() {
        assert_eq!(JsCurrency::usd().inner, RustCurrency::USD);
        assert_eq!(JsCurrency::r#try().inner, RustCurrency::TRY);
        assert_eq!(JsCurrency::jpy().inner.decimals(), 0);
    }
}
