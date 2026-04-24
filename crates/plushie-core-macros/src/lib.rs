//! Derive macros for Plushie widget development.
//!
//! - [`PlushieEnum`]: Make an enum usable as a widget property type.
//!   Variants become snake_case strings on the wire.
//!
//! - [`WidgetEvent`]: Declare widget events as an enum for typed
//!   emission via `EventResult::emit_event()`.
//!
//! - [`WidgetCommand`]: Declare widget commands as an enum for typed
//!   construction via `Command::widget()`.
//!
//! - [`WidgetProps`]: Define your widget's properties as a struct
//!   and get typed extraction from the widget tree.
//!
//! - [`PlushieWidget`]: Generate `type_names` and `fresh_for_session`
//!   for simple stateless widgets.
//!
//! - [`widget!`]: Function-like macro for declaring a custom widget in a
//!   single invocation. Generates the struct, builder, `From<_> for
//!   TreeNode` conversion, and a build-time metadata const.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Lit, parse_macro_input};

// ---------------------------------------------------------------------------
// PlushieEnum derive
// ---------------------------------------------------------------------------

/// Make an enum usable as a widget property type.
///
/// Variants become snake_case strings on the wire. For example,
/// `WordOrGlyph` becomes `"word_or_glyph"`.
///
/// # Example
///
/// ```ignore
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, PlushieEnum)]
/// #[plushie_type(name = "direction")]
/// pub enum Direction {
///     Horizontal,
///     Vertical,
///     Both,
/// }
///
/// // Now usable as a widget property type:
/// // field :direction, Direction
/// // builder: .direction(Direction::Horizontal)
/// ```
///
/// # Custom wire names
///
/// If a variant's wire name doesn't match its snake_case form,
/// override it with `#[plushie(wire = "custom_name")]`. Add
/// alternative names the decoder should accept with
/// `#[plushie(aliases = ["old_name"])]`.
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
        .map(extract_variant_meta)
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
/// Keeps acronym runs together, respects existing underscores, and
/// splits numeric suffixes before the next capitalized word.
fn pascal_to_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            if !result.ends_with('_') && !result.is_empty() {
                result.push('_');
            }
            continue;
        }

        if i > 0 && should_insert_snake_boundary(chars[i - 1], c, chars.get(i + 1).copied()) {
            result.push('_');
        }

        if c.is_uppercase() {
            result.extend(c.to_lowercase());
        } else {
            result.push(c);
        }
    }

    if result.ends_with('_') {
        result.pop();
    }

    result
}

fn should_insert_snake_boundary(prev: char, current: char, next: Option<char>) -> bool {
    if prev == '_' || current == '_' {
        return false;
    }

    let lower_to_upper = prev.is_lowercase() && current.is_uppercase();
    let acronym_to_word =
        prev.is_uppercase() && current.is_uppercase() && next.is_some_and(char::is_lowercase);
    let lower_to_digit =
        prev.is_lowercase() && current.is_ascii_digit() && next.is_some_and(char::is_uppercase);
    let digit_to_word =
        prev.is_ascii_digit() && current.is_uppercase() && next.is_some_and(char::is_lowercase);

    lower_to_upper || acronym_to_word || lower_to_digit || digit_to_word
}

// ---------------------------------------------------------------------------
// WidgetEvent derive
// ---------------------------------------------------------------------------

/// Typed event declarations for composite widgets.
///
/// Generates a `WidgetEventEncode` implementation that converts
/// each variant to a `(family, PropValue)` pair for wire transport.
/// Variant names become snake_case family strings.
///
/// # Variant forms
///
/// - **Unit**: `Cleared` produces `("cleared", PropValue::Null)`
/// - **Single-field tuple**: `Select(u64)` produces `("select", PropValue::U64(v))`
/// - **Named fields**: `Change { x: f32, y: f32 }` produces
///   `("change", PropValue::Object({x: ..., y: ...}))`
///
/// Field types must implement `PlushieType` (all primitives do).
///
/// # Example
///
/// ```ignore
/// #[derive(WidgetEvent)]
/// enum StarRatingEvent {
///     /// User selected a rating.
///     Select(u64),
///     /// Hover state changed.
///     HoverChanged(bool),
///     /// Selection was cleared.
///     Cleared,
/// }
///
/// // In handle_event:
/// EventResult::emit_event(StarRatingEvent::Select(5))
/// ```
#[proc_macro_derive(WidgetEvent)]
pub fn derive_widget_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match derive_widget_event_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_widget_event_impl(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let enum_name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => {
            return Err(syn::Error::new_spanned(
                enum_name,
                "WidgetEvent can only be derived for enums",
            ));
        }
    };

    // Reject multi-field tuple variants (ambiguous encoding).
    for v in variants {
        if let Fields::Unnamed(fields) = &v.fields
            && fields.unnamed.len() > 1
        {
            return Err(syn::Error::new_spanned(
                v,
                "WidgetEvent tuple variants must have exactly one field; \
                 use named fields for multiple values",
            ));
        }
    }

    let match_arms = variants.iter().map(|v| {
        let ident = &v.ident;
        let family = pascal_to_snake(&ident.to_string());

        match &v.fields {
            Fields::Unit => {
                quote! {
                    Self::#ident => (#family, ::plushie_core::protocol::PropValue::Null)
                }
            }
            Fields::Unnamed(_) => {
                // Single-field tuple variant: encode via PlushieType::wire_encode.
                quote! {
                    Self::#ident(v) => (
                        #family,
                        ::plushie_core::types::PlushieType::wire_encode(v),
                    )
                }
            }
            Fields::Named(fields) => {
                let field_names: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();
                let field_keys: Vec<_> = field_names.iter().map(|n| n.to_string()).collect();
                let inserts = field_names
                    .iter()
                    .zip(field_keys.iter())
                    .map(|(name, key)| {
                        quote! {
                            map.insert(
                                #key,
                                ::plushie_core::types::PlushieType::wire_encode(#name),
                            );
                        }
                    });

                quote! {
                    Self::#ident { #(#field_names),* } => {
                        let mut map = ::plushie_core::protocol::PropMap::new();
                        #(#inserts)*
                        (#family, ::plushie_core::protocol::PropValue::Object(map))
                    }
                }
            }
        }
    });

    let spec_arms = generate_spec_arms(variants, "EventSpec", "WidgetEvent")?;

    Ok(quote! {
        impl ::plushie_core::types::WidgetEventEncode for #enum_name {
            fn to_wire(&self) -> (&'static str, ::plushie_core::protocol::PropValue) {
                match self {
                    #(#match_arms,)*
                }
            }
        }

        impl #enum_name {
            /// Return specs for all event variants.
            pub fn event_specs() -> Vec<::plushie_core::spec::EventSpec> {
                vec![#(#spec_arms,)*]
            }
        }
    })
}

// ---------------------------------------------------------------------------
// WidgetCommand derive
// ---------------------------------------------------------------------------

/// Typed command declarations for widget operations.
///
/// Generates a `WidgetCommandEncode` implementation that converts
/// each variant to an `(op, PropValue)` pair for wire transport.
/// Variant names become snake_case operation strings.
///
/// Uses the same variant forms as [`WidgetEvent`]:
///
/// - **Unit**: `Reset` produces `("reset", PropValue::Null)`
/// - **Single-field tuple**: `SetValue(f32)` produces `("set_value", PropValue::F64(v))`
/// - **Named fields**: `SetRange { min: f32, max: f32 }` produces
///   `("set_range", PropValue::Object({min: ..., max: ...}))`
///
/// # Example
///
/// ```ignore
/// #[derive(WidgetCommand)]
/// enum GaugeCommand {
///     /// Set gauge to a value immediately.
///     SetValue(f32),
///     /// Reset gauge to zero.
///     Reset,
///     /// Set the value range.
///     SetRange { min: f32, max: f32 },
/// }
///
/// // Usage:
/// Command::widget("temp", GaugeCommand::SetValue(72.0))
/// ```
#[proc_macro_derive(WidgetCommand)]
pub fn derive_widget_command(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match derive_widget_command_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_widget_command_impl(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let enum_name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => {
            return Err(syn::Error::new_spanned(
                enum_name,
                "WidgetCommand can only be derived for enums",
            ));
        }
    };

    // Reject multi-field tuple variants (ambiguous encoding).
    for v in variants {
        if let Fields::Unnamed(fields) = &v.fields
            && fields.unnamed.len() > 1
        {
            return Err(syn::Error::new_spanned(
                v,
                "WidgetCommand tuple variants must have exactly one field; \
                 use named fields for multiple values",
            ));
        }
    }

    // Generate to_wire() match arms (same logic as WidgetEvent)
    let match_arms = variants.iter().map(|v| {
        let ident = &v.ident;
        let op = pascal_to_snake(&ident.to_string());

        match &v.fields {
            Fields::Unit => {
                quote! {
                    Self::#ident => (#op, ::plushie_core::protocol::PropValue::Null)
                }
            }
            Fields::Unnamed(_) => {
                quote! {
                    Self::#ident(v) => (
                        #op,
                        ::plushie_core::types::PlushieType::wire_encode(v),
                    )
                }
            }
            Fields::Named(fields) => {
                let field_names: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();
                let field_keys: Vec<_> = field_names.iter().map(|n| n.to_string()).collect();
                let inserts = field_names
                    .iter()
                    .zip(field_keys.iter())
                    .map(|(name, key)| {
                        quote! {
                            map.insert(
                                #key,
                                ::plushie_core::types::PlushieType::wire_encode(#name),
                            );
                        }
                    });

                quote! {
                    Self::#ident { #(#field_names),* } => {
                        let mut map = ::plushie_core::protocol::PropMap::new();
                        #(#inserts)*
                        (#op, ::plushie_core::protocol::PropValue::Object(map))
                    }
                }
            }
        }
    });

    let spec_arms = generate_spec_arms(variants, "CommandSpec", "WidgetCommand")?;

    Ok(quote! {
        impl ::plushie_core::spec::WidgetCommandEncode for #enum_name {
            fn to_wire(&self) -> (&'static str, ::plushie_core::protocol::PropValue) {
                match self {
                    #(#match_arms,)*
                }
            }

            fn command_specs() -> Vec<::plushie_core::spec::CommandSpec> {
                vec![#(#spec_arms,)*]
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Shared spec generation for WidgetEvent and WidgetCommand
// ---------------------------------------------------------------------------

/// Generate spec constructor expressions for each enum variant.
///
/// `spec_type` is either "EventSpec" or "CommandSpec".
/// Both use "family" as the name field.
fn generate_spec_arms<'a>(
    variants: impl IntoIterator<Item = &'a syn::Variant>,
    spec_type: &str,
    derive_name: &str,
) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    let spec_ident = format_ident!("{}", spec_type);
    let name_field = format_ident!("family");

    variants
        .into_iter()
        .map(|v| {
            let name = pascal_to_snake(&v.ident.to_string());

            let payload = match &v.fields {
                Fields::Unit => {
                    quote! { ::plushie_core::spec::PayloadSpec::None }
                }
                Fields::Unnamed(fields) => {
                    let ty = &fields.unnamed.first().unwrap().ty;
                    let vt = rust_type_to_value_type(ty, derive_name)?;
                    quote! { ::plushie_core::spec::PayloadSpec::Value(#vt) }
                }
                Fields::Named(fields) => {
                    let field_specs: Vec<_> = fields
                        .named
                        .iter()
                        .map(|f| {
                            let fname = f.ident.as_ref().unwrap().to_string();
                            let vt = rust_type_to_value_type(&f.ty, derive_name)?;
                            Ok(quote! { (#fname.to_string(), #vt) })
                        })
                        .collect::<syn::Result<_>>()?;
                    let required: Vec<_> = fields
                        .named
                        .iter()
                        .map(|f| f.ident.as_ref().unwrap().to_string())
                        .collect();
                    quote! {
                        ::plushie_core::spec::PayloadSpec::Fields {
                            fields: vec![#(#field_specs),*],
                            required: vec![#(#required.to_string()),*],
                        }
                    }
                }
            };

            Ok(quote! {
                ::plushie_core::spec::#spec_ident {
                    #name_field: #name.to_string(),
                    payload: #payload,
                }
            })
        })
        .collect()
}

/// Map a Rust type to a ValueType for spec generation.
fn rust_type_to_value_type(
    ty: &syn::Type,
    derive_name: &str,
) -> syn::Result<proc_macro2::TokenStream> {
    if path_matches(ty, &["f32"]) || path_matches(ty, &["f64"]) {
        return Ok(quote! { ::plushie_core::spec::ValueType::Float });
    }
    if path_matches(ty, &["i32"])
        || path_matches(ty, &["i64"])
        || path_matches(ty, &["u32"])
        || path_matches(ty, &["u64"])
    {
        return Ok(quote! { ::plushie_core::spec::ValueType::Integer });
    }
    if path_matches(ty, &["bool"]) {
        return Ok(quote! { ::plushie_core::spec::ValueType::Bool });
    }
    if path_matches(ty, &["String"])
        || path_matches(ty, &["std", "string", "String"])
        || path_matches(ty, &["alloc", "string", "String"])
    {
        return Ok(quote! { ::plushie_core::spec::ValueType::String });
    }
    if path_matches(ty, &["PropValue"])
        || path_matches(ty, &["plushie_core", "protocol", "PropValue"])
    {
        return Ok(quote! { ::plushie_core::spec::ValueType::Any });
    }

    Err(syn::Error::new_spanned(
        ty,
        format!(
            "unsupported {derive_name} payload type `{}`; supported payload types are f32, f64, i32, i64, u32, u64, bool, String, std::string::String, alloc::string::String, and plushie_core::protocol::PropValue",
            quote!(#ty)
        ),
    ))
}

fn path_matches(ty: &syn::Type, expected: &[&str]) -> bool {
    let syn::Type::Path(type_path) = ty else {
        return false;
    };
    if type_path.qself.is_some() {
        return false;
    }

    let mut segments = type_path.path.segments.iter();
    for expected_ident in expected {
        let Some(segment) = segments.next() else {
            return false;
        };
        if segment.ident != expected_ident {
            return false;
        }
        if !matches!(segment.arguments, syn::PathArguments::None) {
            return false;
        }
    }
    segments.next().is_none()
}

// ---------------------------------------------------------------------------
// WidgetProps derive
// ---------------------------------------------------------------------------

/// Define your widget's properties and get typed extraction.
///
/// Declare your widget's fields as a struct. The derive creates
/// a `{Name}Props` struct with a `from_node()` method that reads
/// each property from the widget tree with the correct type.
///
/// Document fields with `///` comments; they carry over to the
/// generated Props struct.
///
/// # Example
///
/// ```ignore
/// #[derive(WidgetProps)]
/// #[widget(name = "gauge")]
/// struct Gauge {
///     /// Current gauge value (0.0 to 1.0).
///     value: f32,
///     /// Label displayed below the gauge.
///     label: String,
/// }
///
/// // In your widget's render method:
/// let props = GaugeProps::from_node(node);
/// let value = props.value.unwrap_or(0.0);
/// let label = props.label.unwrap_or_default();
/// ```
#[proc_macro_derive(WidgetProps, attributes(widget, field, widget_props))]
pub fn derive_plushie_widget(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match derive_widget_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_widget_impl(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let widget_name = extract_widget_name(input)?;
    let is_container = has_widget_props_container_attr(input);

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

    // Build a field summary for the doc comment
    let field_list: String = fields
        .iter()
        .map(|f| {
            let name = f.ident.as_ref().unwrap().to_string();
            let ty = &f.ty;
            let ty_str = quote!(#ty).to_string();
            // Extract the first doc line if present
            let doc = f
                .attrs
                .iter()
                .filter(|a| a.path().is_ident("doc"))
                .filter_map(|a| {
                    if let syn::Meta::NameValue(nv) = &a.meta
                        && let syn::Expr::Lit(lit) = &nv.value
                        && let syn::Lit::Str(s) = &lit.lit
                    {
                        return Some(s.value().trim().to_string());
                    }
                    None
                })
                .next();
            match doc {
                Some(d) => format!("- **`{}`** (`{}`): {}", name, ty_str, d),
                None => format!("- **`{}`** (`{}`)", name, ty_str),
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let props_doc = format!(
        "Typed properties for the `{}` widget.\n\n## Fields\n\n{}",
        widget_name, field_list
    );
    let from_node_doc = format!("Extract properties from a `{}` tree node.", widget_name);
    let type_name_doc = format!("The widget type name: `\"{}\"`.", widget_name);

    // Second set of extraction tokens for the FromNode impl (quote
    // iterators are consumed on first use).
    let extractions_for_trait = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        let key = name.as_ref().unwrap().to_string();
        quote! {
            #name: <#ty as ::plushie_core::types::PlushieType>::extract(p, #key)
        }
    });

    // -- Builder generation --

    let builder_name = format_ident!("{}Builder", struct_name);

    let builder_setters = fields.iter().map(|f| {
        let name = f.ident.as_ref().unwrap();
        let ty = &f.ty;
        let key = name.to_string();
        let doc = f
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("doc"))
            .filter_map(|a| {
                if let syn::Meta::NameValue(nv) = &a.meta
                    && let syn::Expr::Lit(lit) = &nv.value
                    && let syn::Lit::Str(s) = &lit.lit
                {
                    return Some(s.value().trim().to_string());
                }
                None
            })
            .next();
        let setter_doc = match doc {
            Some(d) => d,
            None => format!("The `{}` property.", key),
        };

        quote! {
            #[doc = #setter_doc]
            pub fn #name(mut self, v: #ty) -> Self {
                self.0.props.insert(
                    #key,
                    ::plushie_core::types::PlushieType::wire_encode(&v),
                );
                self
            }
        }
    });

    let builder_doc = format!(
        "Builder for the `{}` widget.\n\n\
         ## Properties\n\n{}",
        widget_name, field_list
    );
    let builder_new_doc = format!(
        "Create a new `{}` widget builder with the given ID.",
        widget_name
    );
    let builder_fn_doc = format!(
        "Create a `{}` widget builder with the given ID.",
        widget_name
    );

    let container_methods = if is_container {
        quote! {
            /// Append a child node to this container widget.
            pub fn child(mut self, child: ::plushie_core::protocol::TreeNode) -> Self {
                self.0.children.push(child);
                self
            }

            /// Replace all children with the given list.
            pub fn children(mut self, children: ::std::vec::Vec<::plushie_core::protocol::TreeNode>) -> Self {
                self.0.children = children;
                self
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #[doc = #props_doc]
        pub struct #props_name {
            #(#prop_fields,)*
        }

        impl #props_name {
            #[doc = #from_node_doc]
            pub fn from_node(node: &::plushie_core::protocol::TreeNode) -> Self {
                let p = &node.props;
                Self {
                    #(#extractions,)*
                }
            }
        }

        impl ::plushie_core::types::FromNode for #props_name {
            fn from_node(node: &::plushie_core::protocol::TreeNode) -> Self {
                let p = &node.props;
                Self {
                    #(#extractions_for_trait,)*
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
            #[doc = #type_name_doc]
            pub fn type_name() -> &'static str {
                #widget_name
            }

            #[doc = #builder_fn_doc]
            pub fn builder(id: &str) -> #builder_name {
                #builder_name::new(id)
            }
        }

        #[doc = #builder_doc]
        pub struct #builder_name(pub ::plushie_core::WidgetBuilder);

        impl #builder_name {
            #[doc = #builder_new_doc]
            pub fn new(id: &str) -> Self {
                Self(::plushie_core::WidgetBuilder::new(#widget_name, id))
            }

            #(#builder_setters)*

            /// Set a property by key (untyped fallback).
            pub fn prop(mut self, key: &str, value: impl Into<::plushie_core::protocol::PropValue>) -> Self {
                self.0.props.insert(key, value.into());
                self
            }

            #container_methods
        }
    })
}

// ---------------------------------------------------------------------------
// PlushieWidget derive
// ---------------------------------------------------------------------------

/// Generate `type_names` and `fresh_for_session` for a
/// [`PlushieWidget`] impl, and re-declare the impl block with those
/// methods injected.
///
/// Works on unit structs and structs that implement [`Default`] (the
/// derive uses `Self::default()` when the type is not a unit struct).
/// Requires `#[plushie_widget(type_name = "...")]`.
///
/// The derive produces a `PlushieWidget<R>` impl for each renderer
/// where the type also implements `PlushieWidgetRender<R>`. A plain
/// `impl PlushieWidgetRender` targets the default iced renderer;
/// use `impl<R: PlushieRenderer> PlushieWidgetRender<R>` only when
/// the widget needs to stay renderer-generic.
///
/// # Example
///
/// ```ignore
/// use plushie_widget_sdk::prelude::*;
///
/// #[derive(PlushieWidget)]
/// #[plushie_widget(type_name = "my_gauge")]
/// struct MyGauge;
///
/// impl PlushieWidgetRender for MyGauge {
///     fn render<'a>(
///         &'a self,
///         node: &'a TreeNode,
///         ctx: &RenderCtx<'a>,
///     ) -> PlushieElement<'a> {
///         todo!()
///     }
/// }
/// ```
///
/// Stateful widgets that cannot be reached via `Default` should
/// implement `PlushieWidget` manually so the "return a truly fresh
/// instance" contract stays explicit.
#[proc_macro_derive(PlushieWidget, attributes(plushie_widget))]
pub fn derive_plushie_widget_trait(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match derive_plushie_widget_trait_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_plushie_widget_trait_impl(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let type_name = extract_plushie_widget_type_name(input)?;
    let struct_name = &input.ident;

    let is_unit = matches!(
        &input.data,
        Data::Struct(data) if matches!(&data.fields, Fields::Unit)
    );
    let fresh_expr = if is_unit {
        quote! { ::std::boxed::Box::new(Self) }
    } else {
        quote! { ::std::boxed::Box::new(<Self as ::core::default::Default>::default()) }
    };

    Ok(quote! {
        impl<__R: ::plushie_widget_sdk::PlushieRenderer>
            ::plushie_widget_sdk::registry::PlushieWidget<__R> for #struct_name
        where
            Self: ::plushie_widget_sdk::registry::PlushieWidgetRender<__R>,
        {
            fn type_names(&self) -> &[&str] {
                &[#type_name]
            }

            fn render<'a>(
                &'a self,
                node: &'a ::plushie_widget_sdk::protocol::TreeNode,
                ctx: &::plushie_widget_sdk::render_ctx::RenderCtx<'a, __R>,
            ) -> ::plushie_widget_sdk::PlushieElement<'a, __R> {
                <Self as ::plushie_widget_sdk::registry::PlushieWidgetRender<__R>>::render(
                    self, node, ctx,
                )
            }

            fn fresh_for_session(&self)
                -> ::std::boxed::Box<dyn ::plushie_widget_sdk::registry::PlushieWidget<__R>>
            {
                #fresh_expr
            }
        }
    })
}

fn extract_plushie_widget_type_name(input: &DeriveInput) -> syn::Result<String> {
    for attr in &input.attrs {
        if attr.path().is_ident("plushie_widget") {
            let mut name = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("type_name") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        name = Some(s.value());
                        Ok(())
                    } else {
                        Err(meta.error("expected string literal for type_name"))
                    }
                } else {
                    Err(meta.error("unknown plushie_widget attribute, expected `type_name`"))
                }
            })?;
            return name.ok_or_else(|| {
                syn::Error::new_spanned(
                    attr,
                    "plushie_widget attribute requires type_name = \"...\"",
                )
            });
        }
    }
    Err(syn::Error::new_spanned(
        &input.ident,
        "PlushieWidget derive requires #[plushie_widget(type_name = \"...\")] attribute",
    ))
}

fn has_widget_props_container_attr(input: &DeriveInput) -> bool {
    for attr in &input.attrs {
        if attr.path().is_ident("widget_props") {
            let mut is_container = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("container") {
                    is_container = true;
                }
                Ok(())
            });
            if is_container {
                return true;
            }
        }
    }
    false
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
            return name.ok_or_else(|| {
                syn::Error::new_spanned(attr, "widget attribute requires name = \"...\"")
            });
        }
    }
    Err(syn::Error::new_spanned(
        &input.ident,
        "WidgetProps requires #[widget(name = \"...\")] attribute",
    ))
}

// ---------------------------------------------------------------------------
// widget! function-like macro
// ---------------------------------------------------------------------------

/// Declare a custom Plushie widget in one shot.
///
/// Generates the widget struct, builder methods for each field, a
/// [`From<Widget> for TreeNode`] conversion, and a build-time
/// `PLUSHIE_WIDGET_METADATA` constant that `cargo plushie build`
/// reads during native-widget discovery.
///
/// # Cargo.toml metadata
///
/// Widget crates declare themselves via their own `Cargo.toml`. The
/// factory constructor the custom renderer calls at startup lives
/// there, making `Cargo.toml` the single source of truth; the
/// `widget!` attribute carries only the wire-protocol type name.
///
/// ```toml
/// [package.metadata.plushie.widget]
/// type_name = "my_gauge"
/// constructor = "my_gauge::factory::MyGaugeFactory::new()"
/// ```
///
/// The build tool discovers native widgets by scanning the full
/// `cargo metadata` graph for this table. App crates may also carry a
/// complementary table:
///
/// ```toml
/// [package.metadata.plushie]
/// binary_name = "my-app-renderer"        # optional override
/// source_path = "../plushie-rust"        # optional, honored from env too
/// native_widgets = ["my-gauge"]          # optional explicit list
/// ```
///
/// # Example
///
/// ```ignore
/// use plushie_core::widget;
///
/// widget! {
///     /// Circular gauge widget.
///     #[widget(type_name = "my_gauge", crate = "my-gauge")]
///     pub struct Gauge {
///         pub value: f32,
///         pub max: f32,
///         pub color: plushie_core::types::Color,
///     }
///
///     events {
///         ValueChanged(f32),
///     }
/// }
/// ```
///
/// The macro emits:
///
/// - `pub struct Gauge { value: Option<f32>, ... }` with `new(id)` and
///   fluent builder methods (`.value(v)`, `.max(v)`, `.color(c)`).
/// - `impl From<Gauge> for plushie_core::protocol::TreeNode`.
/// - `pub const PLUSHIE_WIDGET_METADATA: &str = "...";` with a JSON
///   snippet describing the widget for the build tool.
/// - An `events { ... }` section expands to a sibling enum with the
///   `WidgetEvent` derive applied.
#[proc_macro]
pub fn widget(input: TokenStream) -> TokenStream {
    let input2: proc_macro2::TokenStream = input.into();
    match widget_impl(input2) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Parsed form of `widget! { ... }` input.
struct WidgetInput {
    attrs: Vec<syn::Attribute>,
    meta: WidgetMeta,
    vis: syn::Visibility,
    ident: syn::Ident,
    fields: syn::FieldsNamed,
    events: Option<WidgetEventsBlock>,
}

/// Fields parsed out of the `#[widget(...)]` attribute.
struct WidgetMeta {
    type_name: String,
    crate_name: Option<String>,
}

/// Parsed `events { ... }` block.
struct WidgetEventsBlock {
    ident: syn::Ident,
    variants: syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
}

impl syn::parse::Parse for WidgetInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse any outer #[doc = "..."] / #[widget(...)] attributes that
        // precede the struct declaration.
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let vis: syn::Visibility = input.parse()?;
        let _struct_token: syn::Token![struct] = input.parse()?;
        let ident: syn::Ident = input.parse()?;
        let fields: syn::FieldsNamed = input.parse()?;

        // Optional trailing `events { ... }` block.
        let events = if input.peek(syn::Ident) {
            let lookahead: syn::Ident = input.fork().parse()?;
            if lookahead == "events" {
                let _events_kw: syn::Ident = input.parse()?;
                let content;
                syn::braced!(content in input);
                let variants = content.parse_terminated(syn::Variant::parse, syn::Token![,])?;
                Some(WidgetEventsBlock {
                    ident: format_ident!("{}Event", ident),
                    variants,
                })
            } else {
                None
            }
        } else {
            None
        };

        let meta = parse_widget_meta(&attrs, &ident)?;

        Ok(WidgetInput {
            attrs,
            meta,
            vis,
            ident,
            fields,
            events,
        })
    }
}

fn parse_widget_meta(attrs: &[syn::Attribute], ident: &syn::Ident) -> syn::Result<WidgetMeta> {
    let mut type_name: Option<String> = None;
    let mut crate_name: Option<String> = None;

    for attr in attrs {
        if !attr.path().is_ident("widget") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("type_name") {
                let value = meta.value()?;
                let lit: Lit = value.parse()?;
                if let Lit::Str(s) = lit {
                    type_name = Some(s.value());
                    Ok(())
                } else {
                    Err(meta.error("type_name must be a string literal"))
                }
            } else if meta.path.is_ident("crate") {
                let value = meta.value()?;
                let lit: Lit = value.parse()?;
                if let Lit::Str(s) = lit {
                    crate_name = Some(s.value());
                    Ok(())
                } else {
                    Err(meta.error("crate must be a string literal"))
                }
            } else if meta.path.is_ident("constructor") {
                // Cargo.toml's `[package.metadata.plushie.widget].constructor`
                // is the single source of truth. Keeping a second copy in
                // the macro attribute invited drift, so the attribute is
                // rejected rather than silently accepted.
                Err(meta.error(
                    "`constructor` is no longer accepted in `#[widget(...)]`; \
                     declare it once in `[package.metadata.plushie.widget]` in \
                     the crate's Cargo.toml",
                ))
            } else {
                Err(meta.error("unknown widget attribute (expected `type_name` or `crate`)"))
            }
        })?;
    }

    let type_name = type_name.ok_or_else(|| {
        syn::Error::new_spanned(
            ident,
            "widget! requires #[widget(type_name = \"...\")] above the struct",
        )
    })?;

    Ok(WidgetMeta {
        type_name,
        crate_name,
    })
}

fn widget_impl(input: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    let parsed: WidgetInput = syn::parse2(input)?;

    let WidgetInput {
        attrs,
        meta,
        vis,
        ident,
        fields,
        events,
    } = parsed;

    // Drop `#[widget(...)]` attrs from the forwarded attribute list;
    // pass through everything else (doc comments, user attributes).
    let pass_attrs: Vec<&syn::Attribute> = attrs
        .iter()
        .filter(|a| !a.path().is_ident("widget"))
        .collect();

    let type_name = &meta.type_name;
    let struct_fields: Vec<(&syn::Ident, &syn::Type, Vec<&syn::Attribute>)> = fields
        .named
        .iter()
        .map(|f| {
            let fname = f.ident.as_ref().expect("named field");
            let ty = &f.ty;
            let docs: Vec<&syn::Attribute> = f
                .attrs
                .iter()
                .filter(|a| a.path().is_ident("doc"))
                .collect();
            (fname, ty, docs)
        })
        .collect();

    // Struct declaration: each declared field becomes Option<T> so
    // builder methods set them one at a time.
    let decl_fields = struct_fields.iter().map(|(fname, ty, docs)| {
        quote! {
            #(#docs)*
            pub #fname: ::core::option::Option<#ty>
        }
    });

    // Default::default() for zero-arg construction (used by `new`).
    let default_inits = struct_fields.iter().map(|(fname, _, _)| {
        quote! { #fname: ::core::option::Option::None }
    });

    // Builder methods. Each typed setter uses `PlushieType::wire_encode`
    // under the hood for `From<Widget> for TreeNode`; here we just store
    // the typed value.
    let builder_methods = struct_fields.iter().map(|(fname, ty, docs)| {
        quote! {
            #(#docs)*
            pub fn #fname(mut self, value: #ty) -> Self {
                self.#fname = ::core::option::Option::Some(value);
                self
            }
        }
    });

    // From<Widget> for TreeNode: encode each Some() field via PlushieType.
    let to_props_inserts = struct_fields.iter().map(|(fname, _, _)| {
        let key = fname.to_string();
        quote! {
            if let ::core::option::Option::Some(v) = widget.#fname {
                props.insert(
                    #key,
                    ::plushie_core::types::PlushieType::wire_encode(&v),
                );
            }
        }
    });

    // JSON metadata for the build tool. Kept plain so the build tool
    // can parse it as a serde_json::Value without extra deps. The
    // `constructor` field is not emitted here: it lives in the crate's
    // Cargo.toml `[package.metadata.plushie.widget]` table, which the
    // build tool reads directly.
    let crate_name_json = match &meta.crate_name {
        Some(c) => format!(",\"crate\":\"{}\"", escape_json(c)),
        None => String::new(),
    };
    let metadata_str = format!(
        "{{\"type_name\":\"{}\",\"struct\":\"{}\"{}}}",
        escape_json(type_name),
        ident,
        crate_name_json,
    );

    // Events block: feed variants through the existing WidgetEvent
    // derive by emitting an enum with the derive attached.
    let events_decl = events.as_ref().map(|e| {
        let ename = &e.ident;
        let variants = e.variants.iter();
        quote! {
            #[derive(::core::fmt::Debug, ::core::clone::Clone, ::plushie_core::WidgetEvent)]
            pub enum #ename {
                #(#variants),*
            }
        }
    });

    // `new(id)` constructor: struct { id, ..defaults }.
    let new_doc = format!("Create a new `{}` widget builder with the given ID.", ident);
    let struct_doc = format!(
        "`{}` widget. Type name: `\"{}\"`. Built by the `widget!` macro.",
        ident, type_name
    );
    let metadata_doc = format!(
        "Build-time metadata for the `{}` widget (consumed by `cargo plushie build`).",
        ident
    );

    Ok(quote! {
        #(#pass_attrs)*
        #[doc = #struct_doc]
        #vis struct #ident {
            /// Widget instance ID (unique within the view tree).
            pub id: ::std::string::String,
            #(#decl_fields,)*
        }

        impl #ident {
            #[doc = #new_doc]
            pub fn new(id: impl ::core::convert::Into<::std::string::String>) -> Self {
                Self {
                    id: id.into(),
                    #(#default_inits,)*
                }
            }

            /// The wire protocol type name this widget maps to.
            pub const fn type_name() -> &'static str {
                #type_name
            }

            #(#builder_methods)*
        }

        impl ::core::convert::From<#ident> for ::plushie_core::protocol::TreeNode {
            fn from(widget: #ident) -> Self {
                let mut props = ::plushie_core::protocol::PropMap::new();
                #(#to_props_inserts)*
                ::plushie_core::protocol::TreeNode {
                    id: widget.id,
                    type_name: #type_name.to_string(),
                    props: ::plushie_core::protocol::Props::from(props),
                    children: ::std::vec::Vec::new(),
                }
            }
        }

        #[doc = #metadata_doc]
        pub const PLUSHIE_WIDGET_METADATA: &::core::primitive::str = #metadata_str;

        #events_decl
    })
}

/// Minimal JSON string escaper: quotes, backslashes, and control chars.
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
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
        assert_eq!(
            pascal_to_snake("ResizingDiagonallyUp"),
            "resizing_diagonally_up"
        );
    }

    #[test]
    fn pascal_to_snake_digits_and_existing_underscores() {
        assert_eq!(pascal_to_snake("GL11Version"), "gl11_version");
        assert_eq!(pascal_to_snake("Version2D"), "version_2d");
        assert_eq!(pascal_to_snake("HTTP2Connection"), "http2_connection");
        assert_eq!(pascal_to_snake("XML_HTTP_Request"), "xml_http_request");
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

    // -- WidgetEvent tests --

    #[test]
    fn widget_event_unit_variant() {
        let input: DeriveInput = parse_quote! {
            enum TestEvent {
                Cleared,
            }
        };
        let output = derive_widget_event_impl(&input).unwrap();
        let output_str = output.to_string();

        assert!(output_str.contains("WidgetEventEncode"));
        assert!(output_str.contains("to_wire"));
        assert!(output_str.contains("\"cleared\""));
        assert!(output_str.contains("PropValue :: Null"));
    }

    #[test]
    fn widget_event_tuple_variant() {
        let input: DeriveInput = parse_quote! {
            enum TestEvent {
                Select(u64),
                HoverChanged(bool),
            }
        };
        let output = derive_widget_event_impl(&input).unwrap();
        let output_str = output.to_string();

        assert!(output_str.contains("\"select\""));
        assert!(output_str.contains("\"hover_changed\""));
        assert!(output_str.contains("wire_encode"));
    }

    #[test]
    fn widget_event_struct_variant() {
        let input: DeriveInput = parse_quote! {
            enum TestEvent {
                Change { x: f32, y: f32 },
            }
        };
        let output = derive_widget_event_impl(&input).unwrap();
        let output_str = output.to_string();

        assert!(output_str.contains("\"change\""));
        assert!(output_str.contains("PropMap"));
        assert!(output_str.contains("\"x\""));
        assert!(output_str.contains("\"y\""));
    }

    #[test]
    fn widget_event_mixed_variants() {
        let input: DeriveInput = parse_quote! {
            enum TestEvent {
                Select(u64),
                Change { x: f32, y: f32 },
                Cleared,
            }
        };
        let output = derive_widget_event_impl(&input).unwrap();
        let output_str = output.to_string();

        assert!(output_str.contains("\"select\""));
        assert!(output_str.contains("\"change\""));
        assert!(output_str.contains("\"cleared\""));
    }

    #[test]
    fn widget_event_rejects_struct() {
        let input: DeriveInput = parse_quote! {
            struct NotAnEnum {
                x: f32,
            }
        };
        assert!(derive_widget_event_impl(&input).is_err());
    }

    #[test]
    fn widget_event_rejects_multi_field_tuple() {
        let input: DeriveInput = parse_quote! {
            enum BadEvent {
                Change(f32, f32),
            }
        };
        assert!(derive_widget_event_impl(&input).is_err());
    }

    #[test]
    fn widget_event_specs_map_qualified_string_types() {
        let input: DeriveInput = parse_quote! {
            enum TestEvent {
                Owned(String),
                Std(std::string::String),
                Alloc(alloc::string::String),
            }
        };
        let output = derive_widget_event_impl(&input).unwrap();
        let output_str = output.to_string();

        assert!(output_str.contains("\"owned\""));
        assert!(output_str.contains("\"std\""));
        assert!(output_str.contains("\"alloc\""));
        assert_eq!(output_str.matches("ValueType :: String").count(), 3);
        assert!(!output_str.contains("ValueType :: Any"));
    }

    #[test]
    fn widget_event_specs_reject_unsupported_payload_type() {
        let input: DeriveInput = parse_quote! {
            enum BadEvent {
                Count(u8),
            }
        };
        let err = derive_widget_event_impl(&input).unwrap_err();
        assert!(
            err.to_string()
                .contains("unsupported WidgetEvent payload type")
        );
    }

    #[test]
    fn widget_command_specs_reject_unsupported_field_type() {
        let input: DeriveInput = parse_quote! {
            enum BadCommand {
                Set { count: usize },
            }
        };
        let err = derive_widget_command_impl(&input).unwrap_err();
        assert!(
            err.to_string()
                .contains("unsupported WidgetCommand payload type")
        );
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
        // from_node inherent method generated
        assert!(output_str.contains("from_node"));
        // FromNode trait impl generated
        assert!(output_str.contains("FromNode"));
        // type_name method generated
        assert!(output_str.contains("\"gauge\""));
        // Field extractions use PlushieType
        assert!(output_str.contains("PlushieType"));

        // Builder struct generated
        assert!(output_str.contains("GaugeBuilder"));
        // Builder wraps WidgetBuilder
        assert!(output_str.contains("WidgetBuilder"));
        // Typed setter methods generated for each field
        assert!(output_str.contains("fn value"));
        assert!(output_str.contains("fn label"));
        assert!(output_str.contains("fn enabled"));
        // builder() static method on the original struct
        assert!(output_str.contains("fn builder"));
        // Untyped fallback prop method
        assert!(output_str.contains("fn prop"));
    }

    #[test]
    fn derive_widget_builder_uses_wire_encode() {
        let input: DeriveInput = parse_quote! {
            #[widget(name = "slider")]
            struct Slider {
                min: f32,
                max: f32,
            }
        };
        let output = derive_widget_impl(&input).unwrap();
        let output_str = output.to_string();

        // Setters encode via PlushieType::wire_encode
        assert!(output_str.contains("wire_encode"));
        // Setters use the field name as the prop key
        assert!(output_str.contains("\"min\""));
        assert!(output_str.contains("\"max\""));
    }

    #[test]
    fn derive_widget_builder_new_uses_widget_name() {
        let input: DeriveInput = parse_quote! {
            #[widget(name = "progress_bar")]
            struct ProgressBar {
                value: f32,
            }
        };
        let output = derive_widget_impl(&input).unwrap();
        let output_str = output.to_string();

        // Builder::new passes the widget name to WidgetBuilder::new
        assert!(output_str.contains("\"progress_bar\""));
        assert!(output_str.contains("ProgressBarBuilder"));
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

    // -- widget! macro tests --

    #[test]
    fn widget_macro_expands() {
        let input: proc_macro2::TokenStream = quote! {
            #[widget(type_name = "my_gauge", crate = "my-gauge")]
            pub struct Gauge {
                pub value: f32,
                pub max: f32,
            }
        };
        let output = widget_impl(input).expect("widget! should expand");
        let s = output.to_string();

        // Struct + ID field.
        assert!(s.contains("pub struct Gauge"));
        assert!(s.contains("pub id :"));
        // Builder methods for declared fields.
        assert!(s.contains("fn value"));
        assert!(s.contains("fn max"));
        // From<Widget> for TreeNode.
        assert!(s.contains("TreeNode"));
        assert!(s.contains("\"my_gauge\""));
        // Metadata const.
        assert!(s.contains("PLUSHIE_WIDGET_METADATA"));
        assert!(s.contains("\\\"type_name\\\""));
        assert!(s.contains("\\\"my_gauge\\\""));
        assert!(s.contains("\\\"crate\\\""));
        // `constructor` lives only in Cargo.toml; never in the macro
        // output.
        assert!(!s.contains("\\\"constructor\\\""));
    }

    #[test]
    fn widget_macro_metadata_is_valid_json() {
        // Drive the JSON assembly directly so the test doesn't have to
        // disentangle escaped string literals from the emitted token
        // stream.
        let type_name = "my_gauge";
        let crate_name_json = format!(",\"crate\":\"{}\"", escape_json("my-gauge"));
        let metadata_str = format!(
            "{{\"type_name\":\"{}\",\"struct\":\"{}\"{}}}",
            escape_json(type_name),
            "Gauge",
            crate_name_json,
        );

        let value: serde_json::Value =
            serde_json::from_str(&metadata_str).expect("metadata parses as JSON");
        assert_eq!(value["type_name"], "my_gauge");
        assert_eq!(value["crate"], "my-gauge");
        assert_eq!(value["struct"], "Gauge");
        assert!(value.get("constructor").is_none());
    }

    #[test]
    fn widget_macro_metadata_without_optional_fields() {
        // Minimal invocation (type_name only) still produces valid JSON.
        let type_name = "bare_widget";
        let metadata_str = format!(
            "{{\"type_name\":\"{}\",\"struct\":\"{}\"{}}}",
            escape_json(type_name),
            "Bare",
            String::new(),
        );
        let value: serde_json::Value =
            serde_json::from_str(&metadata_str).expect("minimal metadata parses as JSON");
        assert_eq!(value["type_name"], "bare_widget");
        assert_eq!(value["struct"], "Bare");
        assert!(value.get("crate").is_none());
        assert!(value.get("constructor").is_none());
    }

    #[test]
    fn widget_macro_rejects_constructor_attribute() {
        let input: proc_macro2::TokenStream = quote! {
            #[widget(type_name = "my_gauge", constructor = "x::y::new()")]
            pub struct Gauge {
                pub value: f32,
            }
        };
        let err = widget_impl(input).expect_err("constructor attribute should be rejected");
        assert!(
            err.to_string().contains("Cargo.toml"),
            "error should point at Cargo.toml: {err}",
        );
    }

    #[test]
    fn escape_json_handles_specials() {
        assert_eq!(escape_json("a\"b"), "a\\\"b");
        assert_eq!(escape_json("a\\b"), "a\\\\b");
        assert_eq!(escape_json("a\nb"), "a\\nb");
        assert_eq!(escape_json("a\tb"), "a\\tb");
        assert_eq!(escape_json("normal_text"), "normal_text");
    }

    #[test]
    fn widget_macro_requires_type_name() {
        let input: proc_macro2::TokenStream = quote! {
            pub struct NoAttr {
                pub value: f32,
            }
        };
        assert!(widget_impl(input).is_err());
    }

    #[test]
    fn widget_macro_with_events_block() {
        let input: proc_macro2::TokenStream = quote! {
            #[widget(type_name = "my_gauge")]
            pub struct Gauge {
                pub value: f32,
            }

            events {
                ValueChanged(f32),
                Cleared,
            }
        };
        let output = widget_impl(input).unwrap().to_string();
        assert!(output.contains("GaugeEvent"));
        assert!(output.contains("WidgetEvent"));
        assert!(output.contains("ValueChanged"));
        assert!(output.contains("Cleared"));
    }
}
