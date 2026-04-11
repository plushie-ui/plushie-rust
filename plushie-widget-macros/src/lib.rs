//! Proc macros for Plushie widget authoring.
//!
//! Provides `#[derive(WidgetProps)]` which generates typed props
//! extraction from a struct annotated with `#[widget(name = "...")]`.
//!
//! # Example
//!
//! ```ignore
//! use plushie_widget_macros::WidgetProps;
//!
//! #[derive(WidgetProps)]
//! #[widget(name = "star_rating")]
//! struct StarRating {
//!     label: String,
//!     size: f32,
//!     visible: bool,
//! }
//! ```
//!
//! This generates:
//!
//! - `StarRatingProps` struct with `Option<T>` fields and a
//!   `from_node(&TreeNode)` method that extracts typed props
//!   via `PlushieType::extract`.
//! - `StarRating::type_name()` returning `"star_rating"`.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Lit, parse_macro_input};

/// Derive macro for Plushie widget prop extraction.
///
/// Annotate a struct with `#[widget(name = "...")]` and derive
/// `WidgetProps` to generate:
///
/// - A `{Name}Props` struct wrapping each field in `Option<T>`,
///   with a `from_node()` method that extracts typed values from
///   a `TreeNode` via `PlushieType::extract`.
/// - A `type_name()` method on the original struct returning the
///   wire protocol type name.
///
/// Fields use `PlushieType::extract` by default. Annotate a field
/// with `#[field(default = <expr>)]` to document the expected
/// default (the attribute is parsed but the default is not used
/// in the Props struct, since all fields are `Option<T>`).
#[proc_macro_derive(WidgetProps, attributes(widget, field))]
pub fn derive_plushie_widget(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match derive_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_impl(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let widget_name = extract_widget_name(input)?;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "WidgetProps requires named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "WidgetProps can only be derived for structs",
            ));
        }
    };

    let struct_name = &input.ident;
    let props_name = format_ident!("{}Props", struct_name);

    // Collect doc attrs from the original struct to reference in the
    // generated props struct doc comment.
    let struct_name_str = struct_name.to_string();

    // Generate props struct fields (all Option<T>).
    let prop_fields = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        let docs = f
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("doc"))
            .collect::<Vec<_>>();
        quote! {
            #(#docs)*
            pub #name: Option<#ty>
        }
    });

    // Generate from_node field extractions.
    let extractions = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        let key = name.as_ref().unwrap().to_string();
        quote! {
            #name: <#ty as ::plushie_core::types::PlushieType>::extract(p, #key)
        }
    });

    let field_names: Vec<_> = fields.iter().map(|f| &f.ident).collect();

    // Generate Debug impl field formatting.
    let debug_fields = field_names.iter().map(|name| {
        let name_str = name.as_ref().unwrap().to_string();
        quote! {
            .field(#name_str, &self.#name)
        }
    });

    let props_doc = format!("Auto-generated props for the `{}` widget.", struct_name_str);

    Ok(quote! {
        #[doc = #props_doc]
        pub struct #props_name {
            #(#prop_fields,)*
        }

        impl #props_name {
            /// Extract typed props from a `TreeNode`.
            pub fn from_node(node: &::plushie_core::protocol::TreeNode) -> Self {
                let p = &node.props;
                Self {
                    #(#extractions,)*
                }
            }
        }

        impl ::core::fmt::Debug for #props_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_struct(stringify!(#props_name))
                    #(#debug_fields)*
                    .finish()
            }
        }

        impl #struct_name {
            /// Returns the widget type name for the wire protocol.
            pub fn type_name() -> &'static str {
                #widget_name
            }
        }
    })
}

fn extract_widget_name(input: &DeriveInput) -> syn::Result<String> {
    for attr in &input.attrs {
        if attr.path().is_ident("widget") {
            let mut name = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        name = Some(s.value());
                        Ok(())
                    } else {
                        Err(meta.error("expected string literal for widget name"))
                    }
                } else {
                    Err(meta.error("unknown widget attribute, expected `name`"))
                }
            })?;
            return name
                .ok_or_else(|| syn::Error::new_spanned(attr, "widget attribute requires name = \"...\""));
        }
    }
    Err(syn::Error::new_spanned(
        &input.ident,
        "WidgetProps requires #[widget(name = \"...\")] attribute",
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn extracts_widget_name() {
        let input: DeriveInput = parse_quote! {
            #[widget(name = "my_widget")]
            struct MyWidget {
                label: String,
            }
        };
        assert_eq!(extract_widget_name(&input).unwrap(), "my_widget");
    }

    #[test]
    fn rejects_missing_widget_attr() {
        let input: DeriveInput = parse_quote! {
            struct NoAttr {
                label: String,
            }
        };
        assert!(extract_widget_name(&input).is_err());
    }

    #[test]
    fn rejects_widget_attr_without_name() {
        let input: DeriveInput = parse_quote! {
            #[widget()]
            struct EmptyAttr {
                label: String,
            }
        };
        assert!(extract_widget_name(&input).is_err());
    }

    #[test]
    fn derive_impl_produces_output() {
        let input: DeriveInput = parse_quote! {
            #[widget(name = "gauge")]
            struct Gauge {
                /// The current value.
                value: f32,
                label: String,
                enabled: bool,
            }
        };
        let output = derive_impl(&input).unwrap();
        let output_str = output.to_string();

        // Props struct generated
        assert!(output_str.contains("GaugeProps"));
        // from_node method generated
        assert!(output_str.contains("from_node"));
        // type_name method generated
        assert!(output_str.contains("\"gauge\""));
        // Field extractions use PlushieType
        assert!(output_str.contains("PlushieType"));
    }

    #[test]
    fn rejects_enum() {
        let input: DeriveInput = parse_quote! {
            #[widget(name = "bad")]
            enum NotAStruct {
                A,
                B,
            }
        };
        assert!(derive_impl(&input).is_err());
    }

    #[test]
    fn rejects_tuple_struct() {
        let input: DeriveInput = parse_quote! {
            #[widget(name = "bad")]
            struct TupleStruct(String, f32);
        };
        assert!(derive_impl(&input).is_err());
    }
}
