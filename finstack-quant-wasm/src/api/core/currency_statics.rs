//! `Currency` static factories, one per ISO-4217 code in
//! `finstack-quant/core/data/iso_4217.csv` (the source of the Rust `Currency`
//! enum). `Currency.usd()` is the twin of the Python module constant
//! `finstack_quant.core.currency.USD`.
//!
//! The list is spelled out so each factory carries its own documentation; the
//! test below fails when it drifts from `Currency::iter()`.

use crate::api::core::currency::JsCurrency;
use finstack_quant_core::currency::Currency as RustCurrency;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_class = Currency)]
impl JsCurrency {
    /// UAE Dirham (`AED`, ISO-4217 numeric 784).
    ///
    /// @returns The `AED` currency.
    #[wasm_bindgen(js_name = "aed")]
    pub fn aed() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::AED,
        }
    }

    /// Afghani (`AFN`, ISO-4217 numeric 971).
    ///
    /// @returns The `AFN` currency.
    #[wasm_bindgen(js_name = "afn")]
    pub fn afn() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::AFN,
        }
    }

    /// Lek (`ALL`, ISO-4217 numeric 8).
    ///
    /// @returns The `ALL` currency.
    #[wasm_bindgen(js_name = "all")]
    pub fn all() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::ALL,
        }
    }

    /// Armenian Dram (`AMD`, ISO-4217 numeric 51).
    ///
    /// @returns The `AMD` currency.
    #[wasm_bindgen(js_name = "amd")]
    pub fn amd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::AMD,
        }
    }

    /// Netherlands Antillean Guilder (`ANG`, ISO-4217 numeric 532).
    ///
    /// @returns The `ANG` currency.
    #[wasm_bindgen(js_name = "ang")]
    pub fn ang() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::ANG,
        }
    }

    /// Kwanza (`AOA`, ISO-4217 numeric 973).
    ///
    /// @returns The `AOA` currency.
    #[wasm_bindgen(js_name = "aoa")]
    pub fn aoa() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::AOA,
        }
    }

    /// Argentine Peso (`ARS`, ISO-4217 numeric 32).
    ///
    /// @returns The `ARS` currency.
    #[wasm_bindgen(js_name = "ars")]
    pub fn ars() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::ARS,
        }
    }

    /// Australian Dollar (`AUD`, ISO-4217 numeric 36).
    ///
    /// @returns The `AUD` currency.
    #[wasm_bindgen(js_name = "aud")]
    pub fn aud() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::AUD,
        }
    }

    /// Aruban Florin (`AWG`, ISO-4217 numeric 533).
    ///
    /// @returns The `AWG` currency.
    #[wasm_bindgen(js_name = "awg")]
    pub fn awg() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::AWG,
        }
    }

    /// Azerbaijan Manat (`AZN`, ISO-4217 numeric 944).
    ///
    /// @returns The `AZN` currency.
    #[wasm_bindgen(js_name = "azn")]
    pub fn azn() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::AZN,
        }
    }

    /// Convertible Mark (`BAM`, ISO-4217 numeric 977).
    ///
    /// @returns The `BAM` currency.
    #[wasm_bindgen(js_name = "bam")]
    pub fn bam() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BAM,
        }
    }

    /// Barbados Dollar (`BBD`, ISO-4217 numeric 52).
    ///
    /// @returns The `BBD` currency.
    #[wasm_bindgen(js_name = "bbd")]
    pub fn bbd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BBD,
        }
    }

    /// Taka (`BDT`, ISO-4217 numeric 50).
    ///
    /// @returns The `BDT` currency.
    #[wasm_bindgen(js_name = "bdt")]
    pub fn bdt() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BDT,
        }
    }

    /// Bulgarian Lev (`BGN`, ISO-4217 numeric 975).
    ///
    /// @returns The `BGN` currency.
    #[wasm_bindgen(js_name = "bgn")]
    pub fn bgn() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BGN,
        }
    }

    /// Bahraini Dinar (`BHD`, ISO-4217 numeric 48).
    ///
    /// @returns The `BHD` currency.
    #[wasm_bindgen(js_name = "bhd")]
    pub fn bhd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BHD,
        }
    }

    /// Burundi Franc (`BIF`, ISO-4217 numeric 108).
    ///
    /// @returns The `BIF` currency.
    #[wasm_bindgen(js_name = "bif")]
    pub fn bif() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BIF,
        }
    }

    /// Bermudian Dollar (`BMD`, ISO-4217 numeric 60).
    ///
    /// @returns The `BMD` currency.
    #[wasm_bindgen(js_name = "bmd")]
    pub fn bmd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BMD,
        }
    }

    /// Brunei Dollar (`BND`, ISO-4217 numeric 96).
    ///
    /// @returns The `BND` currency.
    #[wasm_bindgen(js_name = "bnd")]
    pub fn bnd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BND,
        }
    }

    /// Boliviano (`BOB`, ISO-4217 numeric 68).
    ///
    /// @returns The `BOB` currency.
    #[wasm_bindgen(js_name = "bob")]
    pub fn bob() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BOB,
        }
    }

    /// Brazilian Real (`BRL`, ISO-4217 numeric 986).
    ///
    /// @returns The `BRL` currency.
    #[wasm_bindgen(js_name = "brl")]
    pub fn brl() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BRL,
        }
    }

    /// Bahamian Dollar (`BSD`, ISO-4217 numeric 44).
    ///
    /// @returns The `BSD` currency.
    #[wasm_bindgen(js_name = "bsd")]
    pub fn bsd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BSD,
        }
    }

    /// Ngultrum (`BTN`, ISO-4217 numeric 64).
    ///
    /// @returns The `BTN` currency.
    #[wasm_bindgen(js_name = "btn")]
    pub fn btn() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BTN,
        }
    }

    /// Pula (`BWP`, ISO-4217 numeric 72).
    ///
    /// @returns The `BWP` currency.
    #[wasm_bindgen(js_name = "bwp")]
    pub fn bwp() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BWP,
        }
    }

    /// Belarusian Ruble (`BYN`, ISO-4217 numeric 933).
    ///
    /// @returns The `BYN` currency.
    #[wasm_bindgen(js_name = "byn")]
    pub fn byn() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BYN,
        }
    }

    /// Belize Dollar (`BZD`, ISO-4217 numeric 84).
    ///
    /// @returns The `BZD` currency.
    #[wasm_bindgen(js_name = "bzd")]
    pub fn bzd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::BZD,
        }
    }

    /// Canadian Dollar (`CAD`, ISO-4217 numeric 124).
    ///
    /// @returns The `CAD` currency.
    #[wasm_bindgen(js_name = "cad")]
    pub fn cad() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::CAD,
        }
    }

    /// Congolese Franc (`CDF`, ISO-4217 numeric 976).
    ///
    /// @returns The `CDF` currency.
    #[wasm_bindgen(js_name = "cdf")]
    pub fn cdf() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::CDF,
        }
    }

    /// Swiss Franc (`CHF`, ISO-4217 numeric 756).
    ///
    /// @returns The `CHF` currency.
    #[wasm_bindgen(js_name = "chf")]
    pub fn chf() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::CHF,
        }
    }

    /// Unidad de Fomento (`CLF`, ISO-4217 numeric 990).
    ///
    /// @returns The `CLF` currency.
    #[wasm_bindgen(js_name = "clf")]
    pub fn clf() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::CLF,
        }
    }

    /// Chilean Peso (`CLP`, ISO-4217 numeric 152).
    ///
    /// @returns The `CLP` currency.
    #[wasm_bindgen(js_name = "clp")]
    pub fn clp() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::CLP,
        }
    }

    /// Yuan Renminbi (`CNY`, ISO-4217 numeric 156).
    ///
    /// @returns The `CNY` currency.
    #[wasm_bindgen(js_name = "cny")]
    pub fn cny() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::CNY,
        }
    }

    /// Colombian Peso (`COP`, ISO-4217 numeric 170).
    ///
    /// @returns The `COP` currency.
    #[wasm_bindgen(js_name = "cop")]
    pub fn cop() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::COP,
        }
    }

    /// Costa Rican Colon (`CRC`, ISO-4217 numeric 188).
    ///
    /// @returns The `CRC` currency.
    #[wasm_bindgen(js_name = "crc")]
    pub fn crc() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::CRC,
        }
    }

    /// Peso Convertible (`CUC`, ISO-4217 numeric 931).
    ///
    /// @returns The `CUC` currency.
    #[wasm_bindgen(js_name = "cuc")]
    pub fn cuc() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::CUC,
        }
    }

    /// Cuban Peso (`CUP`, ISO-4217 numeric 192).
    ///
    /// @returns The `CUP` currency.
    #[wasm_bindgen(js_name = "cup")]
    pub fn cup() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::CUP,
        }
    }

    /// Cabo Verde Escudo (`CVE`, ISO-4217 numeric 132).
    ///
    /// @returns The `CVE` currency.
    #[wasm_bindgen(js_name = "cve")]
    pub fn cve() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::CVE,
        }
    }

    /// Czech Koruna (`CZK`, ISO-4217 numeric 203).
    ///
    /// @returns The `CZK` currency.
    #[wasm_bindgen(js_name = "czk")]
    pub fn czk() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::CZK,
        }
    }

    /// Djibouti Franc (`DJF`, ISO-4217 numeric 262).
    ///
    /// @returns The `DJF` currency.
    #[wasm_bindgen(js_name = "djf")]
    pub fn djf() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::DJF,
        }
    }

    /// Danish Krone (`DKK`, ISO-4217 numeric 208).
    ///
    /// @returns The `DKK` currency.
    #[wasm_bindgen(js_name = "dkk")]
    pub fn dkk() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::DKK,
        }
    }

    /// Dominican Peso (`DOP`, ISO-4217 numeric 214).
    ///
    /// @returns The `DOP` currency.
    #[wasm_bindgen(js_name = "dop")]
    pub fn dop() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::DOP,
        }
    }

    /// Algerian Dinar (`DZD`, ISO-4217 numeric 12).
    ///
    /// @returns The `DZD` currency.
    #[wasm_bindgen(js_name = "dzd")]
    pub fn dzd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::DZD,
        }
    }

    /// Egyptian Pound (`EGP`, ISO-4217 numeric 818).
    ///
    /// @returns The `EGP` currency.
    #[wasm_bindgen(js_name = "egp")]
    pub fn egp() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::EGP,
        }
    }

    /// Nakfa (`ERN`, ISO-4217 numeric 232).
    ///
    /// @returns The `ERN` currency.
    #[wasm_bindgen(js_name = "ern")]
    pub fn ern() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::ERN,
        }
    }

    /// Ethiopian Birr (`ETB`, ISO-4217 numeric 230).
    ///
    /// @returns The `ETB` currency.
    #[wasm_bindgen(js_name = "etb")]
    pub fn etb() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::ETB,
        }
    }

    /// Euro (`EUR`, ISO-4217 numeric 978).
    ///
    /// @returns The `EUR` currency.
    #[wasm_bindgen(js_name = "eur")]
    pub fn eur() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::EUR,
        }
    }

    /// Fiji Dollar (`FJD`, ISO-4217 numeric 242).
    ///
    /// @returns The `FJD` currency.
    #[wasm_bindgen(js_name = "fjd")]
    pub fn fjd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::FJD,
        }
    }

    /// Falkland Islands Pound (`FKP`, ISO-4217 numeric 238).
    ///
    /// @returns The `FKP` currency.
    #[wasm_bindgen(js_name = "fkp")]
    pub fn fkp() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::FKP,
        }
    }

    /// Pound Sterling (`GBP`, ISO-4217 numeric 826).
    ///
    /// @returns The `GBP` currency.
    #[wasm_bindgen(js_name = "gbp")]
    pub fn gbp() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::GBP,
        }
    }

    /// Lari (`GEL`, ISO-4217 numeric 981).
    ///
    /// @returns The `GEL` currency.
    #[wasm_bindgen(js_name = "gel")]
    pub fn gel() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::GEL,
        }
    }

    /// Ghana Cedi (`GHS`, ISO-4217 numeric 936).
    ///
    /// @returns The `GHS` currency.
    #[wasm_bindgen(js_name = "ghs")]
    pub fn ghs() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::GHS,
        }
    }

    /// Gibraltar Pound (`GIP`, ISO-4217 numeric 292).
    ///
    /// @returns The `GIP` currency.
    #[wasm_bindgen(js_name = "gip")]
    pub fn gip() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::GIP,
        }
    }

    /// Dalasi (`GMD`, ISO-4217 numeric 270).
    ///
    /// @returns The `GMD` currency.
    #[wasm_bindgen(js_name = "gmd")]
    pub fn gmd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::GMD,
        }
    }

    /// Guinean Franc (`GNF`, ISO-4217 numeric 324).
    ///
    /// @returns The `GNF` currency.
    #[wasm_bindgen(js_name = "gnf")]
    pub fn gnf() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::GNF,
        }
    }

    /// Quetzal (`GTQ`, ISO-4217 numeric 320).
    ///
    /// @returns The `GTQ` currency.
    #[wasm_bindgen(js_name = "gtq")]
    pub fn gtq() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::GTQ,
        }
    }

    /// Guyana Dollar (`GYD`, ISO-4217 numeric 328).
    ///
    /// @returns The `GYD` currency.
    #[wasm_bindgen(js_name = "gyd")]
    pub fn gyd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::GYD,
        }
    }

    /// Hong Kong Dollar (`HKD`, ISO-4217 numeric 344).
    ///
    /// @returns The `HKD` currency.
    #[wasm_bindgen(js_name = "hkd")]
    pub fn hkd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::HKD,
        }
    }

    /// Lempira (`HNL`, ISO-4217 numeric 340).
    ///
    /// @returns The `HNL` currency.
    #[wasm_bindgen(js_name = "hnl")]
    pub fn hnl() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::HNL,
        }
    }

    /// Kuna (`HRK`, ISO-4217 numeric 191).
    ///
    /// @returns The `HRK` currency.
    #[wasm_bindgen(js_name = "hrk")]
    pub fn hrk() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::HRK,
        }
    }

    /// Gourde (`HTG`, ISO-4217 numeric 332).
    ///
    /// @returns The `HTG` currency.
    #[wasm_bindgen(js_name = "htg")]
    pub fn htg() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::HTG,
        }
    }

    /// Forint (`HUF`, ISO-4217 numeric 348).
    ///
    /// @returns The `HUF` currency.
    #[wasm_bindgen(js_name = "huf")]
    pub fn huf() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::HUF,
        }
    }

    /// Rupiah (`IDR`, ISO-4217 numeric 360).
    ///
    /// @returns The `IDR` currency.
    #[wasm_bindgen(js_name = "idr")]
    pub fn idr() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::IDR,
        }
    }

    /// New Israeli Sheqel (`ILS`, ISO-4217 numeric 376).
    ///
    /// @returns The `ILS` currency.
    #[wasm_bindgen(js_name = "ils")]
    pub fn ils() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::ILS,
        }
    }

    /// Indian Rupee (`INR`, ISO-4217 numeric 356).
    ///
    /// @returns The `INR` currency.
    #[wasm_bindgen(js_name = "inr")]
    pub fn inr() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::INR,
        }
    }

    /// Iraqi Dinar (`IQD`, ISO-4217 numeric 368).
    ///
    /// @returns The `IQD` currency.
    #[wasm_bindgen(js_name = "iqd")]
    pub fn iqd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::IQD,
        }
    }

    /// Iranian Rial (`IRR`, ISO-4217 numeric 364).
    ///
    /// @returns The `IRR` currency.
    #[wasm_bindgen(js_name = "irr")]
    pub fn irr() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::IRR,
        }
    }

    /// Iceland Krona (`ISK`, ISO-4217 numeric 352).
    ///
    /// @returns The `ISK` currency.
    #[wasm_bindgen(js_name = "isk")]
    pub fn isk() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::ISK,
        }
    }

    /// Jamaican Dollar (`JMD`, ISO-4217 numeric 388).
    ///
    /// @returns The `JMD` currency.
    #[wasm_bindgen(js_name = "jmd")]
    pub fn jmd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::JMD,
        }
    }

    /// Jordanian Dinar (`JOD`, ISO-4217 numeric 400).
    ///
    /// @returns The `JOD` currency.
    #[wasm_bindgen(js_name = "jod")]
    pub fn jod() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::JOD,
        }
    }

    /// Yen (`JPY`, ISO-4217 numeric 392).
    ///
    /// @returns The `JPY` currency.
    #[wasm_bindgen(js_name = "jpy")]
    pub fn jpy() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::JPY,
        }
    }

    /// Kenyan Shilling (`KES`, ISO-4217 numeric 404).
    ///
    /// @returns The `KES` currency.
    #[wasm_bindgen(js_name = "kes")]
    pub fn kes() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::KES,
        }
    }

    /// Som (`KGS`, ISO-4217 numeric 417).
    ///
    /// @returns The `KGS` currency.
    #[wasm_bindgen(js_name = "kgs")]
    pub fn kgs() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::KGS,
        }
    }

    /// Riel (`KHR`, ISO-4217 numeric 116).
    ///
    /// @returns The `KHR` currency.
    #[wasm_bindgen(js_name = "khr")]
    pub fn khr() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::KHR,
        }
    }

    /// Comorian Franc (`KMF`, ISO-4217 numeric 174).
    ///
    /// @returns The `KMF` currency.
    #[wasm_bindgen(js_name = "kmf")]
    pub fn kmf() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::KMF,
        }
    }

    /// North Korean Won (`KPW`, ISO-4217 numeric 408).
    ///
    /// @returns The `KPW` currency.
    #[wasm_bindgen(js_name = "kpw")]
    pub fn kpw() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::KPW,
        }
    }

    /// Won (`KRW`, ISO-4217 numeric 410).
    ///
    /// @returns The `KRW` currency.
    #[wasm_bindgen(js_name = "krw")]
    pub fn krw() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::KRW,
        }
    }

    /// Kuwaiti Dinar (`KWD`, ISO-4217 numeric 414).
    ///
    /// @returns The `KWD` currency.
    #[wasm_bindgen(js_name = "kwd")]
    pub fn kwd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::KWD,
        }
    }

    /// Cayman Islands Dollar (`KYD`, ISO-4217 numeric 136).
    ///
    /// @returns The `KYD` currency.
    #[wasm_bindgen(js_name = "kyd")]
    pub fn kyd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::KYD,
        }
    }

    /// Tenge (`KZT`, ISO-4217 numeric 398).
    ///
    /// @returns The `KZT` currency.
    #[wasm_bindgen(js_name = "kzt")]
    pub fn kzt() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::KZT,
        }
    }

    /// Lao Kip (`LAK`, ISO-4217 numeric 418).
    ///
    /// @returns The `LAK` currency.
    #[wasm_bindgen(js_name = "lak")]
    pub fn lak() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::LAK,
        }
    }

    /// Lebanese Pound (`LBP`, ISO-4217 numeric 422).
    ///
    /// @returns The `LBP` currency.
    #[wasm_bindgen(js_name = "lbp")]
    pub fn lbp() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::LBP,
        }
    }

    /// Sri Lanka Rupee (`LKR`, ISO-4217 numeric 144).
    ///
    /// @returns The `LKR` currency.
    #[wasm_bindgen(js_name = "lkr")]
    pub fn lkr() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::LKR,
        }
    }

    /// Liberian Dollar (`LRD`, ISO-4217 numeric 430).
    ///
    /// @returns The `LRD` currency.
    #[wasm_bindgen(js_name = "lrd")]
    pub fn lrd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::LRD,
        }
    }

    /// Loti (`LSL`, ISO-4217 numeric 426).
    ///
    /// @returns The `LSL` currency.
    #[wasm_bindgen(js_name = "lsl")]
    pub fn lsl() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::LSL,
        }
    }

    /// Libyan Dinar (`LYD`, ISO-4217 numeric 434).
    ///
    /// @returns The `LYD` currency.
    #[wasm_bindgen(js_name = "lyd")]
    pub fn lyd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::LYD,
        }
    }

    /// Moroccan Dirham (`MAD`, ISO-4217 numeric 504).
    ///
    /// @returns The `MAD` currency.
    #[wasm_bindgen(js_name = "mad")]
    pub fn mad() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MAD,
        }
    }

    /// Moldovan Leu (`MDL`, ISO-4217 numeric 498).
    ///
    /// @returns The `MDL` currency.
    #[wasm_bindgen(js_name = "mdl")]
    pub fn mdl() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MDL,
        }
    }

    /// Malagasy Ariary (`MGA`, ISO-4217 numeric 969).
    ///
    /// @returns The `MGA` currency.
    #[wasm_bindgen(js_name = "mga")]
    pub fn mga() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MGA,
        }
    }

    /// Denar (`MKD`, ISO-4217 numeric 807).
    ///
    /// @returns The `MKD` currency.
    #[wasm_bindgen(js_name = "mkd")]
    pub fn mkd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MKD,
        }
    }

    /// Kyat (`MMK`, ISO-4217 numeric 104).
    ///
    /// @returns The `MMK` currency.
    #[wasm_bindgen(js_name = "mmk")]
    pub fn mmk() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MMK,
        }
    }

    /// Tugrik (`MNT`, ISO-4217 numeric 496).
    ///
    /// @returns The `MNT` currency.
    #[wasm_bindgen(js_name = "mnt")]
    pub fn mnt() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MNT,
        }
    }

    /// Pataca (`MOP`, ISO-4217 numeric 446).
    ///
    /// @returns The `MOP` currency.
    #[wasm_bindgen(js_name = "mop")]
    pub fn mop() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MOP,
        }
    }

    /// Ouguiya (`MRU`, ISO-4217 numeric 929).
    ///
    /// @returns The `MRU` currency.
    #[wasm_bindgen(js_name = "mru")]
    pub fn mru() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MRU,
        }
    }

    /// Mauritius Rupee (`MUR`, ISO-4217 numeric 480).
    ///
    /// @returns The `MUR` currency.
    #[wasm_bindgen(js_name = "mur")]
    pub fn mur() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MUR,
        }
    }

    /// Rufiyaa (`MVR`, ISO-4217 numeric 462).
    ///
    /// @returns The `MVR` currency.
    #[wasm_bindgen(js_name = "mvr")]
    pub fn mvr() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MVR,
        }
    }

    /// Malawi Kwacha (`MWK`, ISO-4217 numeric 454).
    ///
    /// @returns The `MWK` currency.
    #[wasm_bindgen(js_name = "mwk")]
    pub fn mwk() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MWK,
        }
    }

    /// Mexican Peso (`MXN`, ISO-4217 numeric 484).
    ///
    /// @returns The `MXN` currency.
    #[wasm_bindgen(js_name = "mxn")]
    pub fn mxn() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MXN,
        }
    }

    /// Malaysian Ringgit (`MYR`, ISO-4217 numeric 458).
    ///
    /// @returns The `MYR` currency.
    #[wasm_bindgen(js_name = "myr")]
    pub fn myr() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MYR,
        }
    }

    /// Mozambique Metical (`MZN`, ISO-4217 numeric 943).
    ///
    /// @returns The `MZN` currency.
    #[wasm_bindgen(js_name = "mzn")]
    pub fn mzn() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::MZN,
        }
    }

    /// Namibia Dollar (`NAD`, ISO-4217 numeric 516).
    ///
    /// @returns The `NAD` currency.
    #[wasm_bindgen(js_name = "nad")]
    pub fn nad() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::NAD,
        }
    }

    /// Naira (`NGN`, ISO-4217 numeric 566).
    ///
    /// @returns The `NGN` currency.
    #[wasm_bindgen(js_name = "ngn")]
    pub fn ngn() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::NGN,
        }
    }

    /// Cordoba Oro (`NIO`, ISO-4217 numeric 558).
    ///
    /// @returns The `NIO` currency.
    #[wasm_bindgen(js_name = "nio")]
    pub fn nio() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::NIO,
        }
    }

    /// Norwegian Krone (`NOK`, ISO-4217 numeric 578).
    ///
    /// @returns The `NOK` currency.
    #[wasm_bindgen(js_name = "nok")]
    pub fn nok() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::NOK,
        }
    }

    /// Nepalese Rupee (`NPR`, ISO-4217 numeric 524).
    ///
    /// @returns The `NPR` currency.
    #[wasm_bindgen(js_name = "npr")]
    pub fn npr() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::NPR,
        }
    }

    /// New Zealand Dollar (`NZD`, ISO-4217 numeric 554).
    ///
    /// @returns The `NZD` currency.
    #[wasm_bindgen(js_name = "nzd")]
    pub fn nzd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::NZD,
        }
    }

    /// Rial Omani (`OMR`, ISO-4217 numeric 512).
    ///
    /// @returns The `OMR` currency.
    #[wasm_bindgen(js_name = "omr")]
    pub fn omr() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::OMR,
        }
    }

    /// Balboa (`PAB`, ISO-4217 numeric 590).
    ///
    /// @returns The `PAB` currency.
    #[wasm_bindgen(js_name = "pab")]
    pub fn pab() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::PAB,
        }
    }

    /// Sol (`PEN`, ISO-4217 numeric 604).
    ///
    /// @returns The `PEN` currency.
    #[wasm_bindgen(js_name = "pen")]
    pub fn pen() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::PEN,
        }
    }

    /// Kina (`PGK`, ISO-4217 numeric 598).
    ///
    /// @returns The `PGK` currency.
    #[wasm_bindgen(js_name = "pgk")]
    pub fn pgk() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::PGK,
        }
    }

    /// Philippine Peso (`PHP`, ISO-4217 numeric 608).
    ///
    /// @returns The `PHP` currency.
    #[wasm_bindgen(js_name = "php")]
    pub fn php() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::PHP,
        }
    }

    /// Pakistan Rupee (`PKR`, ISO-4217 numeric 586).
    ///
    /// @returns The `PKR` currency.
    #[wasm_bindgen(js_name = "pkr")]
    pub fn pkr() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::PKR,
        }
    }

    /// Zloty (`PLN`, ISO-4217 numeric 985).
    ///
    /// @returns The `PLN` currency.
    #[wasm_bindgen(js_name = "pln")]
    pub fn pln() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::PLN,
        }
    }

    /// Guarani (`PYG`, ISO-4217 numeric 600).
    ///
    /// @returns The `PYG` currency.
    #[wasm_bindgen(js_name = "pyg")]
    pub fn pyg() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::PYG,
        }
    }

    /// Qatari Rial (`QAR`, ISO-4217 numeric 634).
    ///
    /// @returns The `QAR` currency.
    #[wasm_bindgen(js_name = "qar")]
    pub fn qar() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::QAR,
        }
    }

    /// Romanian Leu (`RON`, ISO-4217 numeric 946).
    ///
    /// @returns The `RON` currency.
    #[wasm_bindgen(js_name = "ron")]
    pub fn ron() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::RON,
        }
    }

    /// Serbian Dinar (`RSD`, ISO-4217 numeric 941).
    ///
    /// @returns The `RSD` currency.
    #[wasm_bindgen(js_name = "rsd")]
    pub fn rsd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::RSD,
        }
    }

    /// Russian Ruble (`RUB`, ISO-4217 numeric 643).
    ///
    /// @returns The `RUB` currency.
    #[wasm_bindgen(js_name = "rub")]
    pub fn rub() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::RUB,
        }
    }

    /// Rwanda Franc (`RWF`, ISO-4217 numeric 646).
    ///
    /// @returns The `RWF` currency.
    #[wasm_bindgen(js_name = "rwf")]
    pub fn rwf() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::RWF,
        }
    }

    /// Saudi Riyal (`SAR`, ISO-4217 numeric 682).
    ///
    /// @returns The `SAR` currency.
    #[wasm_bindgen(js_name = "sar")]
    pub fn sar() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SAR,
        }
    }

    /// Solomon Islands Dollar (`SBD`, ISO-4217 numeric 90).
    ///
    /// @returns The `SBD` currency.
    #[wasm_bindgen(js_name = "sbd")]
    pub fn sbd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SBD,
        }
    }

    /// Seychelles Rupee (`SCR`, ISO-4217 numeric 690).
    ///
    /// @returns The `SCR` currency.
    #[wasm_bindgen(js_name = "scr")]
    pub fn scr() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SCR,
        }
    }

    /// Sudanese Pound (`SDG`, ISO-4217 numeric 938).
    ///
    /// @returns The `SDG` currency.
    #[wasm_bindgen(js_name = "sdg")]
    pub fn sdg() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SDG,
        }
    }

    /// Swedish Krona (`SEK`, ISO-4217 numeric 752).
    ///
    /// @returns The `SEK` currency.
    #[wasm_bindgen(js_name = "sek")]
    pub fn sek() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SEK,
        }
    }

    /// Singapore Dollar (`SGD`, ISO-4217 numeric 702).
    ///
    /// @returns The `SGD` currency.
    #[wasm_bindgen(js_name = "sgd")]
    pub fn sgd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SGD,
        }
    }

    /// Saint Helena Pound (`SHP`, ISO-4217 numeric 654).
    ///
    /// @returns The `SHP` currency.
    #[wasm_bindgen(js_name = "shp")]
    pub fn shp() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SHP,
        }
    }

    /// Leone (`SLE`, ISO-4217 numeric 925).
    ///
    /// @returns The `SLE` currency.
    #[wasm_bindgen(js_name = "sle")]
    pub fn sle() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SLE,
        }
    }

    /// Leone (`SLL`, ISO-4217 numeric 694).
    ///
    /// @returns The `SLL` currency.
    #[wasm_bindgen(js_name = "sll")]
    pub fn sll() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SLL,
        }
    }

    /// Somali Shilling (`SOS`, ISO-4217 numeric 706).
    ///
    /// @returns The `SOS` currency.
    #[wasm_bindgen(js_name = "sos")]
    pub fn sos() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SOS,
        }
    }

    /// Surinam Dollar (`SRD`, ISO-4217 numeric 968).
    ///
    /// @returns The `SRD` currency.
    #[wasm_bindgen(js_name = "srd")]
    pub fn srd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SRD,
        }
    }

    /// South Sudanese Pound (`SSP`, ISO-4217 numeric 728).
    ///
    /// @returns The `SSP` currency.
    #[wasm_bindgen(js_name = "ssp")]
    pub fn ssp() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SSP,
        }
    }

    /// Dobra (`STN`, ISO-4217 numeric 930).
    ///
    /// @returns The `STN` currency.
    #[wasm_bindgen(js_name = "stn")]
    pub fn stn() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::STN,
        }
    }

    /// Syrian Pound (`SYP`, ISO-4217 numeric 760).
    ///
    /// @returns The `SYP` currency.
    #[wasm_bindgen(js_name = "syp")]
    pub fn syp() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SYP,
        }
    }

    /// Lilangeni (`SZL`, ISO-4217 numeric 748).
    ///
    /// @returns The `SZL` currency.
    #[wasm_bindgen(js_name = "szl")]
    pub fn szl() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::SZL,
        }
    }

    /// Baht (`THB`, ISO-4217 numeric 764).
    ///
    /// @returns The `THB` currency.
    #[wasm_bindgen(js_name = "thb")]
    pub fn thb() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::THB,
        }
    }

    /// Somoni (`TJS`, ISO-4217 numeric 972).
    ///
    /// @returns The `TJS` currency.
    #[wasm_bindgen(js_name = "tjs")]
    pub fn tjs() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::TJS,
        }
    }

    /// Turkmenistan New Manat (`TMT`, ISO-4217 numeric 934).
    ///
    /// @returns The `TMT` currency.
    #[wasm_bindgen(js_name = "tmt")]
    pub fn tmt() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::TMT,
        }
    }

    /// Tunisian Dinar (`TND`, ISO-4217 numeric 788).
    ///
    /// @returns The `TND` currency.
    #[wasm_bindgen(js_name = "tnd")]
    pub fn tnd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::TND,
        }
    }

    /// Pa'anga (`TOP`, ISO-4217 numeric 776).
    ///
    /// @returns The `TOP` currency.
    #[wasm_bindgen(js_name = "top")]
    pub fn top() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::TOP,
        }
    }

    /// Turkish Lira (`TRY`, ISO-4217 numeric 949).
    ///
    /// @returns The `TRY` currency.
    #[wasm_bindgen(js_name = "try")]
    pub fn r#try() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::TRY,
        }
    }

    /// Trinidad and Tobago Dollar (`TTD`, ISO-4217 numeric 780).
    ///
    /// @returns The `TTD` currency.
    #[wasm_bindgen(js_name = "ttd")]
    pub fn ttd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::TTD,
        }
    }

    /// New Taiwan Dollar (`TWD`, ISO-4217 numeric 901).
    ///
    /// @returns The `TWD` currency.
    #[wasm_bindgen(js_name = "twd")]
    pub fn twd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::TWD,
        }
    }

    /// Tanzanian Shilling (`TZS`, ISO-4217 numeric 834).
    ///
    /// @returns The `TZS` currency.
    #[wasm_bindgen(js_name = "tzs")]
    pub fn tzs() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::TZS,
        }
    }

    /// Hryvnia (`UAH`, ISO-4217 numeric 980).
    ///
    /// @returns The `UAH` currency.
    #[wasm_bindgen(js_name = "uah")]
    pub fn uah() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::UAH,
        }
    }

    /// Uganda Shilling (`UGX`, ISO-4217 numeric 800).
    ///
    /// @returns The `UGX` currency.
    #[wasm_bindgen(js_name = "ugx")]
    pub fn ugx() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::UGX,
        }
    }

    /// US Dollar (`USD`, ISO-4217 numeric 840).
    ///
    /// @returns The `USD` currency.
    #[wasm_bindgen(js_name = "usd")]
    pub fn usd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::USD,
        }
    }

    /// Peso Uruguayo (`UYU`, ISO-4217 numeric 858).
    ///
    /// @returns The `UYU` currency.
    #[wasm_bindgen(js_name = "uyu")]
    pub fn uyu() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::UYU,
        }
    }

    /// Uzbekistan Sum (`UZS`, ISO-4217 numeric 860).
    ///
    /// @returns The `UZS` currency.
    #[wasm_bindgen(js_name = "uzs")]
    pub fn uzs() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::UZS,
        }
    }

    /// Bolívar Soberano (`VED`, ISO-4217 numeric 926).
    ///
    /// @returns The `VED` currency.
    #[wasm_bindgen(js_name = "ved")]
    pub fn ved() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::VED,
        }
    }

    /// Bolívar Soberano (`VES`, ISO-4217 numeric 928).
    ///
    /// @returns The `VES` currency.
    #[wasm_bindgen(js_name = "ves")]
    pub fn ves() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::VES,
        }
    }

    /// Dong (`VND`, ISO-4217 numeric 704).
    ///
    /// @returns The `VND` currency.
    #[wasm_bindgen(js_name = "vnd")]
    pub fn vnd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::VND,
        }
    }

    /// Vatu (`VUV`, ISO-4217 numeric 548).
    ///
    /// @returns The `VUV` currency.
    #[wasm_bindgen(js_name = "vuv")]
    pub fn vuv() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::VUV,
        }
    }

    /// Tala (`WST`, ISO-4217 numeric 882).
    ///
    /// @returns The `WST` currency.
    #[wasm_bindgen(js_name = "wst")]
    pub fn wst() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::WST,
        }
    }

    /// CFA Franc BEAC (`XAF`, ISO-4217 numeric 950).
    ///
    /// @returns The `XAF` currency.
    #[wasm_bindgen(js_name = "xaf")]
    pub fn xaf() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::XAF,
        }
    }

    /// East Caribbean Dollar (`XCD`, ISO-4217 numeric 951).
    ///
    /// @returns The `XCD` currency.
    #[wasm_bindgen(js_name = "xcd")]
    pub fn xcd() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::XCD,
        }
    }

    /// CFA Franc BCEAO (`XOF`, ISO-4217 numeric 952).
    ///
    /// @returns The `XOF` currency.
    #[wasm_bindgen(js_name = "xof")]
    pub fn xof() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::XOF,
        }
    }

    /// CFP Franc (`XPF`, ISO-4217 numeric 953).
    ///
    /// @returns The `XPF` currency.
    #[wasm_bindgen(js_name = "xpf")]
    pub fn xpf() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::XPF,
        }
    }

    /// Yemeni Rial (`YER`, ISO-4217 numeric 886).
    ///
    /// @returns The `YER` currency.
    #[wasm_bindgen(js_name = "yer")]
    pub fn yer() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::YER,
        }
    }

    /// Rand (`ZAR`, ISO-4217 numeric 710).
    ///
    /// @returns The `ZAR` currency.
    #[wasm_bindgen(js_name = "zar")]
    pub fn zar() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::ZAR,
        }
    }

    /// Zambian Kwacha (`ZMW`, ISO-4217 numeric 967).
    ///
    /// @returns The `ZMW` currency.
    #[wasm_bindgen(js_name = "zmw")]
    pub fn zmw() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::ZMW,
        }
    }

    /// Zimbabwe Dollar (`ZWL`, ISO-4217 numeric 932).
    ///
    /// @returns The `ZWL` currency.
    #[wasm_bindgen(js_name = "zwl")]
    pub fn zwl() -> JsCurrency {
        JsCurrency {
            inner: RustCurrency::ZWL,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    /// Codes with a static factory above, in file order.
    const FACTORY_CODES: &[&str] = &[
        "AED", "AFN", "ALL", "AMD", "ANG", "AOA", "ARS", "AUD", "AWG", "AZN", "BAM", "BBD", "BDT",
        "BGN", "BHD", "BIF", "BMD", "BND", "BOB", "BRL", "BSD", "BTN", "BWP", "BYN", "BZD", "CAD",
        "CDF", "CHF", "CLF", "CLP", "CNY", "COP", "CRC", "CUC", "CUP", "CVE", "CZK", "DJF", "DKK",
        "DOP", "DZD", "EGP", "ERN", "ETB", "EUR", "FJD", "FKP", "GBP", "GEL", "GHS", "GIP", "GMD",
        "GNF", "GTQ", "GYD", "HKD", "HNL", "HRK", "HTG", "HUF", "IDR", "ILS", "INR", "IQD", "IRR",
        "ISK", "JMD", "JOD", "JPY", "KES", "KGS", "KHR", "KMF", "KPW", "KRW", "KWD", "KYD", "KZT",
        "LAK", "LBP", "LKR", "LRD", "LSL", "LYD", "MAD", "MDL", "MGA", "MKD", "MMK", "MNT", "MOP",
        "MRU", "MUR", "MVR", "MWK", "MXN", "MYR", "MZN", "NAD", "NGN", "NIO", "NOK", "NPR", "NZD",
        "OMR", "PAB", "PEN", "PGK", "PHP", "PKR", "PLN", "PYG", "QAR", "RON", "RSD", "RUB", "RWF",
        "SAR", "SBD", "SCR", "SDG", "SEK", "SGD", "SHP", "SLE", "SLL", "SOS", "SRD", "SSP", "STN",
        "SYP", "SZL", "THB", "TJS", "TMT", "TND", "TOP", "TRY", "TTD", "TWD", "TZS", "UAH", "UGX",
        "USD", "UYU", "UZS", "VED", "VES", "VND", "VUV", "WST", "XAF", "XCD", "XOF", "XPF", "YER",
        "ZAR", "ZMW", "ZWL",
    ];

    #[test]
    fn every_currency_has_a_static_factory() {
        let expected: Vec<String> = RustCurrency::iter().map(|c| c.to_string()).collect();
        assert_eq!(
            FACTORY_CODES, expected,
            "regenerate currency_statics.rs from finstack-quant/core/data/iso_4217.csv"
        );
    }

    #[test]
    fn factories_return_their_own_code() {
        assert_eq!(JsCurrency::usd().inner, RustCurrency::USD);
        assert_eq!(JsCurrency::r#try().inner, RustCurrency::TRY);
        assert_eq!(JsCurrency::jpy().inner.decimals(), 0);
    }
}
