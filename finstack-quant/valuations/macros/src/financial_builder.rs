//! FinancialBuilder derive macro for generating type-safe builder patterns.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{parse_macro_input, Data, DeriveInput, Expr, Fields};

/// Implementation of the FinancialBuilder derive macro.
///
/// Automatically generates a type-safe builder with:
/// - Required fields (non-Option types must be set)
/// - Optional fields (Option<T> or marked with #[builder(optional)])
/// - Validation on build (e.g., start_date < maturity)
/// - Ergonomic setter methods
///
/// # Builder pattern: no-args entry point
///
/// The generated `builder()` method takes **no arguments**. All fields —
/// including required ones — are set through individual setter methods.
/// This is an intentional design choice for instrument types where the number
/// of required fields is large (e.g. `Bond` has 7+ required fields).
/// Passing them all as positional arguments would be error-prone and
/// unreadable; setters give each value a name and allow any call order.
///
/// This differs from the hand-written builders in `finstack-quant-core` for
/// market-data curves (e.g. `DiscountCurve::builder(id)`), which accept a
/// single required key — the curve identifier — as a constructor argument.
/// Curves have a natural unique key and few other required parameters,
/// making the args-based entry point both practical and ergonomic.
///
/// **Summary of the two patterns:**
///
/// | Layer | Pattern | Example | Rationale |
/// |-------|---------|---------|-----------|
/// | `finstack-quant-core` curves | `Type::builder(id)` | `DiscountCurve::builder("USD-OIS")` | Curves have a single natural key; remaining params have sensible defaults. |
/// | `finstack-quant-valuations` instruments | `Type::builder()` | `Bond::builder().id(id).notional(n)…` | Many required fields; named setters are clearer than positional args. |
///
/// # Example
///
/// ```text
/// #[derive(FinancialBuilder)]
/// pub struct Bond {
///     pub id: InstrumentId,
///     pub notional: Money,
///     pub coupon: f64,
///     pub maturity: Date,
///     pub attributes: Attributes,
/// }
///
/// // Generated builder usage:
/// let bond = Bond::builder()
///     .id(id)
///     .notional(notional)
///     .coupon(0.05)
///     .maturity(maturity)
///     .build()?;
/// ```
pub(crate) fn derive_financial_builder_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = input.ident.clone();

    // Collect fields and builder annotations
    let mut required_fields: Vec<(syn::Ident, syn::Type)> = Vec::new();
    let mut optional_fields: Vec<(syn::Ident, syn::Type)> = Vec::new();
    let mut defaults: HashMap<syn::Ident, Expr> = HashMap::new();
    let mut custom_validator: Option<Expr> = None;
    // Heuristics for post-build validations
    let mut has_start_date: bool = false;
    let mut has_issue: bool = false;
    let mut issue_field_ident: Option<syn::Ident> = None;
    let mut has_maturity: bool = false;
    let mut has_strike_variance: bool = false;
    let mut has_optional_notional: bool = false;
    let mut has_optional_spot_rate: bool = false;
    let mut has_base_currency: bool = false;
    let mut has_quote_currency: bool = false;
    let mut has_instrument_pricing_overrides: bool = false;
    let mut has_metric_pricing_overrides: bool = false;
    let mut has_scenario_pricing_overrides: bool = false;

    for attr in &input.attrs {
        if attr.path().is_ident("builder") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("validate") {
                    custom_validator = Some(meta.value()?.parse()?);
                }
                Ok(())
            });
        }
    }

    let fields = match input.data {
        Data::Struct(s) => s.fields,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "FinancialBuilder can only be derived for structs",
            )
            .to_compile_error()
            .into()
        }
    };

    // By default, treat Option<T> as optional; #[builder(optional)] is honored only when field type is Option<...>
    if let Fields::Named(named) = fields {
        for f in named.named {
            let Some(ident) = f.ident else {
                return syn::Error::new_spanned(&f.ty, "Named field should have an identifier")
                    .to_compile_error()
                    .into();
            };
            let ty = f.ty.clone();

            let mut has_optional_attr = false;
            let mut default_expr: Option<Expr> = None;
            for attr in f.attrs {
                if attr.path().is_ident("builder") {
                    let _ = attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("optional") {
                            has_optional_attr = true;
                        } else if meta.path.is_ident("default") {
                            // Support #[builder(default)] and #[builder(default = <expr>)]
                            if meta.input.peek(syn::Token![=]) {
                                let value: Expr = meta.value()?.parse()?;
                                default_expr = Some(value);
                            } else {
                                // Use type's Default::default()
                                let ty_default: Expr =
                                    syn::parse_quote! { ::core::default::Default::default() };
                                default_expr = Some(ty_default);
                            }
                        }
                        Ok(())
                    });
                }
            }

            if let Some(expr) = default_expr {
                defaults.insert(ident.clone(), expr);
            }

            let is_option_ty = matches!(ty, syn::Type::Path(ref tp) if tp.path.segments.last().map(|s| s.ident == "Option").unwrap_or(false));

            // Track presence of well-known fields for generic validations
            if ident == format_ident!("start_date") {
                has_start_date = true;
            }
            if ident == format_ident!("issue") || ident == format_ident!("issue_date") {
                has_issue = true;
                issue_field_ident = Some(ident.clone());
            }
            if ident == format_ident!("maturity") || ident == format_ident!("maturity_date") {
                has_maturity = true;
            }
            if ident == format_ident!("strike_variance") {
                has_strike_variance = true;
            }
            if ident == format_ident!("base_currency") {
                has_base_currency = true;
            }
            if ident == format_ident!("quote_currency") {
                has_quote_currency = true;
            }
            if ident == format_ident!("instrument_pricing_overrides") {
                has_instrument_pricing_overrides = true;
            }
            if ident == format_ident!("metric_pricing_overrides") {
                has_metric_pricing_overrides = true;
            }
            if ident == format_ident!("scenario_pricing_overrides") {
                has_scenario_pricing_overrides = true;
            }
            if ident == format_ident!("notional") && is_option_ty {
                has_optional_notional = true;
            }
            if ident == format_ident!("spot_rate") && is_option_ty {
                has_optional_spot_rate = true;
            }
            // Optional if Option<T> or the field is `attributes`
            if is_option_ty || ident == format_ident!("attributes") {
                optional_fields.push((ident, ty));
            } else {
                required_fields.push((ident, ty));
            }
        }
    } else {
        return syn::Error::new_spanned(&struct_name, "FinancialBuilder requires named fields")
            .to_compile_error()
            .into();
    }

    let builder_name = format_ident!("{}Builder", struct_name);

    // Builder struct fields are Option<...> for required, and same type for optional if already Option<T>, else Option<T>
    let builder_req_fields = required_fields
        .iter()
        .map(|(id, ty)| quote! { #id: ::core::option::Option<#ty> });
    let builder_opt_fields = optional_fields.iter().map(|(id, ty)| {
        if let syn::Type::Path(ref tp) = ty {
            if tp
                .path
                .segments
                .last()
                .map(|s| s.ident == "Option")
                .unwrap_or(false)
            {
                // Keep Option<T> as is
                quote! { #id: #ty }
            } else {
                quote! { #id: ::core::option::Option<#ty> }
            }
        } else {
            quote! { #id: ::core::option::Option<#ty> }
        }
    });

    // Setter methods
    let setter_req = required_fields.iter().map(|(id, ty)| {
        let doc_text = format!("Sets the `{}` field.", id);
        quote! {
            #[doc = #doc_text]
            pub fn #id(mut self, value: #ty) -> Self { self.#id = ::core::option::Option::Some(value); self }
        }
    });

    let setter_opt = optional_fields.iter().map(|(id, ty)| {
        let doc_text = format!("Sets the `{}` field.", id);
        let doc_text_opt = format!("Sets the `{}` field as an option.", id);
        if let syn::Type::Path(ref tp) = ty {
            if let Some(seg) = tp.path.segments.last() {
                if seg.ident == "Option" {
                    // Extract inner type Option<Inner>
                    let inner_ty: Option<syn::Type> = match &seg.arguments {
                        syn::PathArguments::AngleBracketed(ab) => ab.args.iter().find_map(|ga| {
                            if let syn::GenericArgument::Type(t) = ga { Some(t.clone()) } else { None }
                        }),
                        _ => None,
                    };
                    if let Some(inner) = inner_ty {
                        let set_opt = format_ident!("{}_opt", id);
                        quote! {
                            #[doc = #doc_text]
                            pub fn #id(mut self, value: #inner) -> Self { self.#id = ::core::option::Option::Some(value); self }
                            #[doc = #doc_text_opt]
                            pub fn #set_opt(mut self, value: #ty) -> Self { self.#id = value; self }
                        }
                    } else {
                        quote! {
                            #[doc = #doc_text]
                            pub fn #id(mut self, value: #ty) -> Self { self.#id = value; self }
                        }
                    }
                } else {
                    quote! {
                        #[doc = #doc_text]
                        pub fn #id(mut self, value: #ty) -> Self { self.#id = ::core::option::Option::Some(value); self }
                    }
                }
            } else {
                quote! {
                    #[doc = #doc_text]
                    pub fn #id(mut self, value: #ty) -> Self { self.#id = ::core::option::Option::Some(value); self }
                }
            }
        } else {
            quote! {
                #[doc = #doc_text]
                pub fn #id(mut self, value: #ty) -> Self { self.#id = ::core::option::Option::Some(value); self }
            }
        }
    });

    // Resolve the maturity field identifier for issue-date fallback generation.
    let maturity_field_ident = if required_fields.iter().any(|(id, _)| id == "maturity")
        || optional_fields.iter().any(|(id, _)| id == "maturity")
    {
        Some(format_ident!("maturity"))
    } else if required_fields.iter().any(|(id, _)| id == "maturity_date")
        || optional_fields.iter().any(|(id, _)| id == "maturity_date")
    {
        Some(format_ident!("maturity_date"))
    } else {
        None
    };

    // Build expression: required fields unwrap, optional fields carry through (unwrap_or(None)) and initialize attributes if present
    let assign_req = required_fields.iter().map(|(id, _)| {
        if let Some(expr) = defaults.get(id) {
            let expr_clone = expr.clone();
            quote! { #id: self.#id.unwrap_or(#expr_clone) }
        } else if issue_field_ident.as_ref() == Some(id) {
            if let Some(ref mat_ident) = maturity_field_ident {
                quote! {
                    #id: match self.#id {
                        ::core::option::Option::Some(d) => d,
                        ::core::option::Option::None => {
                            let mat = self.#mat_ident.ok_or(finstack_quant_core::InputError::Invalid)?;
                            mat.checked_sub(::time::Duration::days(365)).unwrap_or(mat)
                        }
                    }
                }
            } else {
                quote! { #id: self.#id.ok_or(finstack_quant_core::InputError::Invalid)? }
            }
        } else {
            quote! { #id: self.#id.ok_or(finstack_quant_core::InputError::Invalid)? }
        }
    });

    let assign_opt = optional_fields.iter().map(|(id, ty)| {
        if let syn::Type::Path(ref tp) = ty {
            if tp
                .path
                .segments
                .last()
                .map(|s| s.ident == "Option")
                .unwrap_or(false)
            {
                quote! { #id: self.#id }
            } else if id == "attributes" {
                quote! { attributes: self.attributes.unwrap_or_default() }
            } else {
                quote! { #id: self.#id.unwrap_or_default() }
            }
        } else if id == "attributes" {
            quote! { attributes: self.attributes.unwrap_or_default() }
        } else {
            quote! { #id: self.#id.unwrap_or_default() }
        }
    });

    // Post-build validation snippets based on field presence
    let mut post_build_checks = proc_macro2::TokenStream::new();
    if has_start_date && has_maturity {
        // Check if the struct has maturity or maturity_date field
        let maturity_field = if required_fields.iter().any(|(id, _)| id == "maturity")
            || optional_fields.iter().any(|(id, _)| id == "maturity")
        {
            quote! { __built.maturity }
        } else {
            quote! { __built.maturity_date }
        };
        post_build_checks.extend(quote! {
            if __built.start_date >= #maturity_field {
                return ::core::result::Result::Err(finstack_quant_core::InputError::InvalidDateRange.into());
            }
        });
    }
    // Also validate issue < maturity for instruments with issue date
    if has_issue && has_maturity {
        if let Some(issue_field) = issue_field_ident.as_ref() {
            let maturity_field = if required_fields.iter().any(|(id, _)| id == "maturity")
                || optional_fields.iter().any(|(id, _)| id == "maturity")
            {
                quote! { __built.maturity }
            } else {
                quote! { __built.maturity_date }
            };
            post_build_checks.extend(quote! {
                if __built.#issue_field >= #maturity_field {
                    return ::core::result::Result::Err(finstack_quant_core::InputError::InvalidDateRange.into());
                }
            });
        }
    }
    if has_strike_variance {
        post_build_checks.extend(quote! {
            if __built.strike_variance < 0.0 {
                return ::core::result::Result::Err(finstack_quant_core::InputError::NegativeValue.into());
            }
        });
    }
    if has_optional_notional && has_optional_spot_rate {
        post_build_checks.extend(quote! {
            if __built.notional.is_none() && __built.spot_rate.is_none() {
                return ::core::result::Result::Err(finstack_quant_core::Error::Validation(
                    "Builder validation failed: at least one of `notional` or `spot_rate` must be set".to_string()
                ));
            }
        });
    }
    if has_base_currency && has_quote_currency {
        post_build_checks.extend(quote! {
            if __built.base_currency == __built.quote_currency {
                return ::core::result::Result::Err(finstack_quant_core::Error::Validation(format!(
                    "Builder validation failed: base_currency ({}) must differ from quote_currency ({})",
                    __built.base_currency,
                    __built.quote_currency
                )));
            }
        });
    }
    if let Some(validator) = custom_validator {
        post_build_checks.extend(quote! {
            #validator(&__built)?;
        });
    }

    let builder_doc = format!("Builder for `{}`.", struct_name);
    let new_doc = "Creates a new builder instance.";
    let build_doc = "Builds the final instance.";
    let builder_method_doc = "Creates a new builder.";
    let focused_override_compatibility = (has_instrument_pricing_overrides
        && has_metric_pricing_overrides
        && has_scenario_pricing_overrides)
        .then(|| {
            quote! {
                /// Sets focused runtime fields from the temporary compatibility bag.
                pub fn pricing_overrides(
                    mut self,
                    value: crate::instruments::PricingOverrides,
                ) -> Self {
                    self.instrument_pricing_overrides =
                        ::core::option::Option::Some(value.instrument);
                    self.metric_pricing_overrides =
                        ::core::option::Option::Some(value.metrics);
                    self.scenario_pricing_overrides =
                        ::core::option::Option::Some(value.scenario);
                    self
                }
            }
        });

    let expanded = quote! {
        #[doc = #builder_doc]
        #[allow(non_camel_case_types)]
        #[derive(Default)]
        pub struct #builder_name {
            #(#builder_req_fields,)*
            #(#builder_opt_fields,)*
        }

        impl #builder_name {
            #[doc = #new_doc]
            pub fn new() -> Self { Self::default() }
            #(#setter_req)*
            #(#setter_opt)*
            #focused_override_compatibility

            #[doc = #build_doc]
            pub fn build(self) -> finstack_quant_core::Result<#struct_name> {
                let __built = #struct_name {
                    #(#assign_req,)*
                    #(#assign_opt,)*
                };
                // Generic sanity checks for common domain fields
                #post_build_checks
                Ok(__built)
            }
        }

        impl #struct_name {
            #[doc = #builder_method_doc]
            #[must_use]
            pub fn builder() -> #builder_name { #builder_name::new() }
        }
    };

    TokenStream::from(expanded)
}
