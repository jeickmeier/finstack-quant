//! FinancialBuilder derive macro for generating type-safe builder patterns.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{parse_macro_input, Data, DeriveInput, Expr, Fields, Lit, Meta};

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
    let mut field_docs: HashMap<syn::Ident, String> = HashMap::new();

    let mut custom_validator: Option<Expr> = None;
    for attr in &input.attrs {
        if attr.path().is_ident("builder") {
            let parsed = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("validate") {
                    custom_validator = Some(meta.value()?.parse()?);
                }
                Ok(())
            });
            if let Err(error) = parsed {
                return error.to_compile_error().into();
            }
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

            let docs = f
                .attrs
                .iter()
                .filter_map(|attr| {
                    if !attr.path().is_ident("doc") {
                        return None;
                    }
                    match &attr.meta {
                        Meta::NameValue(value) => match &value.value {
                            Expr::Lit(expr) => match &expr.lit {
                                Lit::Str(text) => Some(text.value().trim().to_owned()),
                                _ => None,
                            },
                            _ => None,
                        },
                        _ => None,
                    }
                })
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let docs = if docs.is_empty() {
                format!("Replacement value for `{ident}`.")
            } else {
                docs
            };
            field_docs.insert(ident.clone(), docs);

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
        let summary = format!("Sets the `{id}` field.");
        let argument = field_docs
            .get(id)
            .cloned()
            .unwrap_or_else(|| format!("Replacement value for `{id}`."));
        let doc_text = format!("{summary}\n\n# Arguments\n\n* `value` - {argument}");
        quote! {
            #[doc = #doc_text]
            #[must_use]
            pub fn #id(mut self, value: #ty) -> Self { self.#id = ::core::option::Option::Some(value); self }
        }
    });

    let setter_opt = optional_fields.iter().map(|(id, ty)| {
        let argument = field_docs
            .get(id)
            .cloned()
            .unwrap_or_else(|| format!("Replacement value for `{id}`."));
        let doc_text = format!("Sets the `{id}` field.\n\n# Arguments\n\n* `value` - {argument}");
        let doc_text_opt = format!(
            "Sets the `{id}` field from an optional value.\n\n# Arguments\n\n* `value` - Optional field value. Pass `None` to leave `{id}` unset."
        );
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
                            #[must_use]
                            pub fn #id(mut self, value: #inner) -> Self { self.#id = ::core::option::Option::Some(value); self }
                            #[doc = #doc_text_opt]
                            #[must_use]
                            pub fn #set_opt(mut self, value: #ty) -> Self { self.#id = value; self }
                        }
                    } else {
                        quote! {
                            #[doc = #doc_text]
                            #[must_use]
                            pub fn #id(mut self, value: #ty) -> Self { self.#id = value; self }
                        }
                    }
                } else {
                    quote! {
                        #[doc = #doc_text]
                        #[must_use]
                        pub fn #id(mut self, value: #ty) -> Self { self.#id = ::core::option::Option::Some(value); self }
                    }
                }
            } else {
                quote! {
                    #[doc = #doc_text]
                    #[must_use]
                    pub fn #id(mut self, value: #ty) -> Self { self.#id = ::core::option::Option::Some(value); self }
                }
            }
        } else {
            quote! {
                #[doc = #doc_text]
                #[must_use]
                pub fn #id(mut self, value: #ty) -> Self { self.#id = ::core::option::Option::Some(value); self }
            }
        }
    });

    // Build expression: required fields unwrap, optional fields carry through
    // (unwrap_or(None)) and initialize attributes if present. A missing required
    // field fails with a validation error naming the builder and the field.
    let builder_name_str = builder_name.to_string();
    let missing_error = |field: &syn::Ident| {
        let message = format!("{builder_name_str}: missing required field '{field}'");
        quote! { finstack_quant_core::Error::Validation(::std::string::String::from(#message)) }
    };
    let assign_req = required_fields.iter().map(|(id, _)| {
        if let Some(expr) = defaults.get(id) {
            let expr_clone = expr.clone();
            quote! { #id: self.#id.unwrap_or(#expr_clone) }
        } else {
            let missing = missing_error(id);
            quote! { #id: self.#id.ok_or_else(|| #missing)? }
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

    let builder_doc = format!("Builder for `{}`.", struct_name);
    let new_doc = "Creates a new builder instance.";
    let build_doc = "Builds the final instance.";
    let builder_method_doc = "Creates a new builder.";
    let validation = if let Some(validator) = custom_validator {
        quote! { #validator(&__built)?; }
    } else {
        quote! {
            finstack_quant_valuations::instruments::Instrument::validate_invariants(&__built)?;
        }
    };
    let expanded = quote! {
        #[doc = #builder_doc]
        #[must_use]
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
            #[doc = #build_doc]
            pub fn build(self) -> finstack_quant_core::Result<#struct_name> {
                let __built = #struct_name {
                    #(#assign_req,)*
                    #(#assign_opt,)*
                };
                #validation
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
