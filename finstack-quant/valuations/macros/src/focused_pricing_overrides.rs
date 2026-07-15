//! Serde/schema bridge for focused runtime pricing-override fields.

use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{parse_macro_input, Data, DeriveInput, Fields, LitStr, Path};

const INSTRUMENT_FIELD: &str = "instrument_pricing_overrides";
const METRIC_FIELD: &str = "metric_pricing_overrides";
const SCENARIO_FIELD: &str = "scenario_pricing_overrides";

pub(crate) fn derive_focused_pricing_overrides_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let struct_name = input.ident;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            input.generics,
            "FocusedPricingOverrides does not support generic structs",
        ));
    }
    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "FocusedPricingOverrides requires named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &struct_name,
                "FocusedPricingOverrides can only be derived for structs",
            ))
        }
    };

    let mut ordinary = Vec::new();
    let mut instrument_ident = None;
    let mut metric_ident = None;
    let mut scenario_ident = None;
    for field in fields {
        let ident = field.ident.clone().ok_or_else(|| {
            syn::Error::new_spanned(&field, "FocusedPricingOverrides requires named fields")
        })?;
        match ident.to_string().as_str() {
            INSTRUMENT_FIELD => instrument_ident = Some(ident),
            METRIC_FIELD => metric_ident = Some(ident),
            SCENARIO_FIELD => scenario_ident = Some(ident),
            _ => ordinary.push(field),
        }
    }

    let instrument_ident = required_field(instrument_ident, INSTRUMENT_FIELD, &struct_name)?;
    let metric_ident = required_field(metric_ident, METRIC_FIELD, &struct_name)?;
    let scenario_ident = required_field(scenario_ident, SCENARIO_FIELD, &struct_name)?;
    let try_from = parse_try_from(&input.attrs)?;
    let skip_deserialize = has_pricing_flag(&input.attrs, "skip_deserialize");
    let deny_unknown_fields = has_serde_flag(&input.attrs, "deny_unknown_fields");
    let serde_container = deny_unknown_fields.then(|| quote!(#[serde(deny_unknown_fields)]));

    let ordinary_idents: Vec<_> = ordinary
        .iter()
        .map(|field| {
            field.ident.as_ref().ok_or_else(|| {
                syn::Error::new_spanned(field, "FocusedPricingOverrides requires named fields")
            })
        })
        .collect::<syn::Result<_>>()?;
    let ordinary_types: Vec<_> = ordinary.iter().map(|field| &field.ty).collect();
    let ordinary_serde_attrs: Vec<Vec<_>> = ordinary
        .iter()
        .map(|field| {
            field
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("serde"))
                .collect()
        })
        .collect();
    let ordinary_schema_attrs: Vec<Vec<_>> = ordinary
        .iter()
        .map(|field| {
            field
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("serde") || attr.path().is_ident("schemars"))
                .collect()
        })
        .collect();

    let deserialize_target = if let Some(path) = try_from {
        quote! {
            let unchecked = #path {
                #(#ordinary_idents: shadow.#ordinary_idents,)*
                #instrument_ident: shadow.pricing_overrides.instrument,
                #metric_ident: shadow.pricing_overrides.metrics,
                #scenario_ident: shadow.pricing_overrides.scenario,
            };
            <Self as std::convert::TryFrom<#path>>::try_from(unchecked)
                .map_err(serde::de::Error::custom)
        }
    } else {
        quote! {
            Ok(Self {
                #(#ordinary_idents: shadow.#ordinary_idents,)*
                #instrument_ident: shadow.pricing_overrides.instrument,
                #metric_ident: shadow.pricing_overrides.metrics,
                #scenario_ident: shadow.pricing_overrides.scenario,
            })
        }
    };

    let deserialize_impl = (!skip_deserialize).then(|| {
        quote! {
            impl<'de> serde::Deserialize<'de> for #struct_name {
                fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
                where
                    D: serde::Deserializer<'de>,
                {
                    #[derive(serde::Deserialize)]
                    #[allow(dead_code)]
                    #serde_container
                    struct Shadow {
                        #(
                            #(#ordinary_serde_attrs)*
                            #ordinary_idents: #ordinary_types,
                        )*
                        #[serde(
                            default,
                            deserialize_with =
                                "crate::instruments::common_impl::parameters::deserialize_null_default"
                        )]
                        pricing_overrides:
                            crate::instruments::pricing_overrides::PricingOverridesWire,
                    }

                    let shadow = <Shadow as serde::Deserialize>::deserialize(deserializer)?;
                    #deserialize_target
                }
            }
        }
    });
    let schema_shadow = format_ident!("__{}FocusedPricingOverridesSchema", struct_name);
    Ok(quote! {
        impl serde::Serialize for #struct_name {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                #[derive(serde::Serialize)]
                #[allow(dead_code)]
                #serde_container
                struct Shadow<'a> {
                    #(
                        #(#ordinary_serde_attrs)*
                        #ordinary_idents: &'a #ordinary_types,
                    )*
                    #[serde(default)]
                    pricing_overrides:
                        crate::instruments::pricing_overrides::PricingOverridesWire,
                }

                serde::Serialize::serialize(&Shadow {
                    #(#ordinary_idents: &self.#ordinary_idents,)*
                    pricing_overrides:
                        crate::instruments::pricing_overrides::PricingOverridesWire {
                            instrument: self.#instrument_ident.clone(),
                            metrics: self.#metric_ident.clone(),
                            scenario: self.#scenario_ident.clone(),
                        },
                }, serializer)
            }
        }

        #deserialize_impl

        impl schemars::JsonSchema for #struct_name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                std::borrow::Cow::Borrowed(stringify!(#struct_name))
            }

            fn json_schema(
                generator: &mut schemars::SchemaGenerator,
            ) -> schemars::Schema {
                #[derive(schemars::JsonSchema)]
                #[allow(dead_code)]
                #serde_container
                struct #schema_shadow {
                    #(
                        #(#ordinary_schema_attrs)*
                        #ordinary_idents: #ordinary_types,
                    )*
                    #[serde(default)]
                    pricing_overrides:
                        crate::instruments::pricing_overrides::PricingOverridesWire,
                }

                <#schema_shadow as schemars::JsonSchema>::json_schema(generator)
            }
        }
    })
}

fn required_field(
    ident: Option<syn::Ident>,
    field: &str,
    struct_name: &syn::Ident,
) -> syn::Result<syn::Ident> {
    ident.ok_or_else(|| {
        syn::Error::new_spanned(
            struct_name,
            format!("FocusedPricingOverrides requires a `{field}` field"),
        )
    })
}

fn parse_try_from(attrs: &[syn::Attribute]) -> syn::Result<Option<Path>> {
    let mut result = None;
    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("pricing_overrides"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("try_from") {
                let value: LitStr = meta.value()?.parse()?;
                result = Some(value.parse()?);
                Ok(())
            } else if meta.path.is_ident("skip_deserialize") {
                Ok(())
            } else {
                Err(meta.error("unsupported pricing_overrides option"))
            }
        })?;
    }
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("try_from") {
                let value: LitStr = meta.value()?.parse()?;
                result = Some(value.parse()?);
            }
            Ok(())
        })?;
    }
    Ok(result)
}

fn has_pricing_flag(attrs: &[syn::Attribute], flag: &str) -> bool {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("pricing_overrides"))
        .any(|attr| attr.meta.to_token_stream().to_string().contains(flag))
}

fn has_serde_flag(attrs: &[syn::Attribute], flag: &str) -> bool {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("serde"))
        .any(|attr| attr.meta.to_token_stream().to_string().contains(flag))
}
