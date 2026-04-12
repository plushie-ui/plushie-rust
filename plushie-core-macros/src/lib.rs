//! Proc macros for Plushie types and widgets.
//!
//! Two derives:
//!
//! - `PlushieEnum`: generates `PlushieType` impl for simple
//!   string-encoded enums. Used by plushie-core's own enum types.
//!
//! - `WidgetProps`: generates typed prop extraction from a struct
//!   annotated with `#[widget(name = "...")]`. Used by widget
//!   authors for native widgets.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Lit, parse_macro_input};

// ---------------------------------------------------------------------------
// PlushieEnum derive
// ---------------------------------------------------------------------------

/// Derive `PlushieType` for a simple string-encoded enum.
///
/// Annotate the enum with `#[plushie_type(name = "...")]` to set the
/// wire type name. Variants map to snake_case by default. Override
/// with `#[plushie(wire = "custom")]` on individual variants. Add
/// decode aliases with `#[plushie(aliases = ["a", "b"])]`.
///
/// # Example
///
/// ```ignore
/// #[derive(PlushieEnum)]
/// #[plushie_type(name = "direction")]
/// pub enum Direction {
///     Horizontal,
///     Vertical,
///     Both,
/// }
/// ```
///
/// Generates `wire_decode`, `wire_encode`, `extract`, and `type_name`.
#[proc_macro_derive(PlushieEnum, attributes(plushie_type, plushie))]
pub fn derive_plushie_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match derive_enum_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Per-variant metadata extracted from attributes.
struct VariantMeta {
    ident: syn::Ident,
    wire_name: String,
    aliases: Vec<String>,
}

fn derive_enum_impl(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let type_name = extract_plushie_type_name(input)?;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "PlushieEnum can only be derived for enums",
            ));
        }
    };

    // Reject variants with fields (tuple or struct variants).
    for v in variants {
        if !matches!(v.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                v,
                "PlushieEnum requires all variants to be unit variants (no fields)",
            ));
        }
    }

    let metas: Vec<VariantMeta> = variants
        .iter()
        .map(|v| extract_variant_meta(v))
        .collect::<syn::Result<_>>()?;

    let enum_name = &input.ident;

    // wire_decode match arms: canonical name + aliases
    let decode_arms = metas.iter().map(|m| {
        let ident = &m.ident;
        let wire = &m.wire_name;
        let alias_pats = m.aliases.iter().map(|a| quote! { | #a });
        quote! {
            #wire #(#alias_pats)* => ::core::option::Option::Some(Self::#ident)
        }
    });

    // wire_encode match arms
    let encode_arms = metas.iter().map(|m| {
        let ident = &m.ident;
        let wire = &m.wire_name;
        quote! {
            Self::#ident => #wire
        }
    });

    // extract match arms (same as decode but for &str from Props)
    let extract_arms = metas.iter().map(|m| {
        let ident = &m.ident;
        let wire = &m.wire_name;
        let alias_pats = m.aliases.iter().map(|a| quote! { | #a });
        quote! {
            #wire #(#alias_pats)* => ::core::option::Option::Some(Self::#ident)
        }
    });

    Ok(quote! {
        impl ::plushie_core::types::PlushieType for #enum_name {
            fn wire_decode(value: &::serde_json::Value) -> ::core::option::Option<Self> {
                match value.as_str()? {
                    #(#decode_arms,)*
                    _ => ::core::option::Option::None,
                }
            }

            fn wire_encode(&self) -> ::plushie_core::protocol::PropValue {
                ::plushie_core::protocol::PropValue::Str(
                    match self {
                        #(#encode_arms,)*
                    }
                    .into(),
                )
            }

            fn extract(
                props: &::plushie_core::protocol::Props,
                key: &str,
            ) -> ::core::option::Option<Self> {
                match props.get_str(key)? {
                    #(#extract_arms,)*
                    _ => ::core::option::Option::None,
                }
            }

            fn type_name() -> &'static str {
                #type_name
            }
        }
    })
}

fn extract_plushie_type_name(input: &DeriveInput) -> syn::Result<String> {
    for attr in &input.attrs {
        if attr.path().is_ident("plushie_type") {
            let mut name = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        name = Some(s.value());
                        Ok(())
                    } else {
                        Err(meta.error("expected string literal for plushie_type name"))
                    }
                } else {
                    Err(meta.error("unknown plushie_type attribute, expected `name`"))
                }
            })?;
            return name.ok_or_else(|| {
                syn::Error::new_spanned(attr, "plushie_type attribute requires name = \"...\"")
            });
        }
    }
    Err(syn::Error::new_spanned(
        &input.ident,
        "PlushieEnum requires #[plushie_type(name = \"...\")] attribute",
    ))
}

fn extract_variant_meta(variant: &syn::Variant) -> syn::Result<VariantMeta> {
    let ident = variant.ident.clone();
    let mut wire_name: Option<String> = None;
    let mut aliases: Vec<String> = Vec::new();

    for attr in &variant.attrs {
        if attr.path().is_ident("plushie") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("wire") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        wire_name = Some(s.value());
                        Ok(())
                    } else {
                        Err(meta.error("expected string literal for wire name"))
                    }
                } else if meta.path.is_ident("aliases") {
                    let value = meta.value()?;
                    let array: syn::ExprArray = value.parse()?;
                    for elem in &array.elems {
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: Lit::Str(s), ..
                        }) = elem
                        {
                            aliases.push(s.value());
                        } else {
                            return Err(syn::Error::new_spanned(
                                elem,
                                "expected string literal in aliases array",
                            ));
                        }
                    }
                    Ok(())
                } else {
                    Err(meta.error("unknown plushie attribute, expected `wire` or `aliases`"))
                }
            })?;
        }
    }

    let wire_name = wire_name.unwrap_or_else(|| pascal_to_snake(&ident.to_string()));

    Ok(VariantMeta {
        ident,
        wire_name,
        aliases,
    })
}

/// Convert PascalCase to snake_case.
///
/// Inserts `_` before each uppercase letter that follows a lowercase
/// letter or precedes a lowercase letter in a run of uppercase.
fn pascal_to_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                if prev.is_lowercase() {
                    // camelCase boundary: aB -> a_b
                    result.push('_');
                } else if prev.is_uppercase() {
                    // Check if this uppercase starts a new word:
                    // ABc -> a_bc (the B starts the word "Bc")
                    if i + 1 < chars.len() && chars[i + 1].is_lowercase() {
                        result.push('_');
                    }
                }
            }
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }

    result
}

// ---------------------------------------------------------------------------
// WidgetProps derive
// ---------------------------------------------------------------------------

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

    match derive_widget_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_widget_impl(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
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
    use syn::{DeriveInput, parse_quote};

    // -- PlushieEnum tests --

    #[test]
    fn pascal_to_snake_simple() {
        assert_eq!(pascal_to_snake("None"), "none");
        assert_eq!(pascal_to_snake("Word"), "word");
        assert_eq!(pascal_to_snake("WordOrGlyph"), "word_or_glyph");
        assert_eq!(pascal_to_snake("AlwaysOnTop"), "always_on_top");
        assert_eq!(pascal_to_snake("ScaleDown"), "scale_down");
    }

    #[test]
    fn pascal_to_snake_consecutive_upper() {
        assert_eq!(pascal_to_snake("URL"), "url");
        assert_eq!(pascal_to_snake("HTMLParser"), "html_parser");
        assert_eq!(pascal_to_snake("ResizingDiagonallyUp"), "resizing_diagonally_up");
    }

    #[test]
    fn pascal_to_snake_single_char() {
        assert_eq!(pascal_to_snake("X"), "x");
        assert_eq!(pascal_to_snake("Y"), "y");
    }

    #[test]
    fn extract_plushie_type_name_works() {
        let input: DeriveInput = parse_quote! {
            #[plushie_type(name = "direction")]
            enum Direction {
                Horizontal,
                Vertical,
            }
        };
        assert_eq!(extract_plushie_type_name(&input).unwrap(), "direction");
    }

    #[test]
    fn rejects_missing_plushie_type() {
        let input: DeriveInput = parse_quote! {
            enum NoAttr {
                A,
            }
        };
        assert!(extract_plushie_type_name(&input).is_err());
    }

    #[test]
    fn variant_meta_default_wire_name() {
        let input: DeriveInput = parse_quote! {
            #[plushie_type(name = "test")]
            enum Test {
                WordOrGlyph,
            }
        };
        if let Data::Enum(data) = &input.data {
            let meta = extract_variant_meta(&data.variants[0]).unwrap();
            assert_eq!(meta.wire_name, "word_or_glyph");
            assert!(meta.aliases.is_empty());
        }
    }

    #[test]
    fn variant_meta_custom_wire_and_aliases() {
        let input: DeriveInput = parse_quote! {
            #[plushie_type(name = "test")]
            enum Test {
                #[plushie(wire = "table_row", aliases = ["row"])]
                Row,
            }
        };
        if let Data::Enum(data) = &input.data {
            let meta = extract_variant_meta(&data.variants[0]).unwrap();
            assert_eq!(meta.wire_name, "table_row");
            assert_eq!(meta.aliases, vec!["row"]);
        }
    }

    #[test]
    fn derive_enum_impl_produces_output() {
        let input: DeriveInput = parse_quote! {
            #[plushie_type(name = "direction")]
            enum Direction {
                Horizontal,
                Vertical,
                Both,
            }
        };
        let output = derive_enum_impl(&input).unwrap();
        let output_str = output.to_string();

        assert!(output_str.contains("PlushieType"));
        assert!(output_str.contains("wire_decode"));
        assert!(output_str.contains("wire_encode"));
        assert!(output_str.contains("\"horizontal\""));
        assert!(output_str.contains("\"direction\""));
    }

    #[test]
    fn rejects_struct_for_enum_derive() {
        let input: DeriveInput = parse_quote! {
            #[plushie_type(name = "bad")]
            struct NotAnEnum {
                x: f32,
            }
        };
        assert!(derive_enum_impl(&input).is_err());
    }

    #[test]
    fn rejects_tuple_variant() {
        let input: DeriveInput = parse_quote! {
            #[plushie_type(name = "bad")]
            enum HasData {
                A(i32),
            }
        };
        assert!(derive_enum_impl(&input).is_err());
    }

    // -- WidgetProps tests --

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
    fn derive_widget_impl_produces_output() {
        let input: DeriveInput = parse_quote! {
            #[widget(name = "gauge")]
            struct Gauge {
                /// The current value.
                value: f32,
                label: String,
                enabled: bool,
            }
        };
        let output = derive_widget_impl(&input).unwrap();
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
    fn rejects_enum_for_widget() {
        let input: DeriveInput = parse_quote! {
            #[widget(name = "bad")]
            enum NotAStruct {
                A,
                B,
            }
        };
        assert!(derive_widget_impl(&input).is_err());
    }

    #[test]
    fn rejects_tuple_struct_for_widget() {
        let input: DeriveInput = parse_quote! {
            #[widget(name = "bad")]
            struct TupleStruct(String, f32);
        };
        assert!(derive_widget_impl(&input).is_err());
    }
}
