//! Procedural macros for the `finstack-quant-wasm` bindings.
//!
//! The crate holds one attribute, [`macro@plain_docs`]. wasm-bindgen copies an
//! export's doc comments into the generated TypeScript declarations and JSDoc,
//! the published documentation of the WASM package. When a `macro_rules!`
//! invocation forwards `///` comments into the item it generates, rustc hands
//! them on as raw string literals (`#[doc = r" text"]`), and wasm-bindgen
//! 0.2.130 keeps the `r"`/`r#"` prefix in its output. `#[plain_docs]` rewrites
//! every `#[doc = <string>]` on the item and its members as an ordinary string
//! literal with the same value before `#[wasm_bindgen]` reads them, and folds
//! `#[doc = concat!(...)]` over string literals, which wasm-bindgen would
//! otherwise drop.
//!
//! # Examples
//!
//! ```ignore
//! macro_rules! getter {
//!     ($(#[$doc:meta])* $name:ident) => {
//!         #[finstack_quant_wasm_macros::plain_docs]
//!         #[wasm_bindgen(js_class = Thing)]
//!         impl JsThing {
//!             $(#[$doc])*
//!             pub fn $name(&self) -> f64 { self.inner }
//!         }
//!     };
//! }
//! ```

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::visit_mut::VisitMut;

/// Rewrites `#[doc = <string literal>]` attributes as ordinary string literals.
struct PlainDocs;

impl VisitMut for PlainDocs {
    fn visit_attribute_mut(&mut self, attr: &mut syn::Attribute) {
        let syn::Meta::NameValue(name_value) = &mut attr.meta else {
            return;
        };
        if !name_value.path.is_ident("doc") {
            return;
        }
        let text = match &name_value.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(text),
                ..
            }) => text.clone(),
            syn::Expr::Macro(call) if call.mac.path.is_ident("concat") => {
                match concat_literals(&call.mac) {
                    Some(text) => text,
                    None => return,
                }
            }
            _ => return,
        };
        name_value.value = syn::Expr::Lit(syn::ExprLit {
            attrs: Vec::new(),
            lit: syn::Lit::Str(syn::LitStr::new(&text.value(), text.span())),
        });
    }
}

/// Folds `concat!` over string literals into one literal; `None` when an
/// argument is anything else. wasm-bindgen does not expand macros in doc
/// attributes, so an unfolded `concat!` doc would be dropped from its output.
fn concat_literals(mac: &syn::Macro) -> Option<syn::LitStr> {
    let parts = mac
        .parse_body_with(
            syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
        )
        .ok()?;
    let span = parts.first()?.span();
    let value: String = parts.iter().map(syn::LitStr::value).collect();
    Some(syn::LitStr::new(&value, span))
}

/// Rewrites the doc comments of the annotated item, and of every item, member
/// and field inside it, as ordinary string literals.
///
/// Place it before `#[wasm_bindgen]` on an item a `macro_rules!` body emits
/// with forwarded doc comments; attribute macros expand outermost first, so
/// wasm-bindgen then reads plain literals. A `concat!` of string literals is
/// folded into one literal; any other doc value is left unchanged.
///
/// # Arguments
///
/// * `args` - Must be empty; the attribute takes no options.
/// * `item` - The item whose doc attributes are rewritten.
///
/// # Returns
///
/// The item with its doc attributes rewritten, or a compile error when `args`
/// is not empty or `item` is not an item.
#[proc_macro_attribute]
pub fn plain_docs(args: TokenStream, item: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(
            proc_macro::Span::call_site().into(),
            "`plain_docs` takes no arguments",
        )
        .to_compile_error()
        .into();
    }
    let mut item = syn::parse_macro_input!(item as syn::Item);
    PlainDocs.visit_item_mut(&mut item);
    item.into_token_stream().into()
}
