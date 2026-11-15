//! `#[derive(Identifier)]`: an identifier type written as one path, in every
//! form it takes (JSON, the store, Studio keys), so a typed economy written
//! down is a `Paths` economy.
//!
//! A variant is its kebab-case name followed by its fields in order
//! (`Move { from: Start, to: Exit }` is `move/start/exit`); a struct is its
//! fields in order (`JobId(3)` is `3`), and a unit struct its name. Fields are
//! identifiers, unsigned integers, `bool` or `Option` of one (`none`, or
//! `some` and the value).
//!
//! Beside the type it writes its typed patterns, `NamePattern`: the same
//! variants and fields, each field an `axionomy::Slot` of its type, which
//! converts into `axionomy::Pattern<Name>`. An enum whose variants have no
//! fields is written in one segment, so a variable can take its values;
//! the derive lists them.
//!
//! It also derives `Ord` as the natural order of those paths: variants by
//! name, then fields in order, numbers by value. So a typed identifier and its
//! path sort alike, and a typed economy written down is a `Paths` economy byte
//! for byte. Derive `Eq`, `PartialEq` and `Hash` as usual, but not `Ord` or
//! `PartialOrd`.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, parse_macro_input, spanned::Spanned};

#[proc_macro_derive(Identifier)]
pub fn identifier(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(&input, Whole::Identifier)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// `#[derive(Segments)]`: only the path a value is written as, for a type
/// that keeps its own order and serialization, as types written in a
/// fixture's numbers do. `#[derive(Identifier)]` is this, ordered and
/// serialized as its path.
#[proc_macro_derive(Segments)]
pub fn segments(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(&input, Whole::Segments)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// What a derive makes of a type: its path only, or the identifier whole.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Whole {
    Segments,
    Identifier,
}

fn expand(input: &DeriveInput, whole: Whole) -> syn::Result<Tokens> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.generics.span(),
            "an identifier has no type parameters",
        ));
    }
    let name = &input.ident;
    let (write, read) = match &input.data {
        Data::Enum(data) => {
            let mut seen = std::collections::BTreeMap::new();
            let mut writes = Vec::new();
            let mut reads = Vec::new();
            for variant in &data.variants {
                let segment = kebab(&variant.ident);
                if let Some(other) = seen.insert(segment.clone(), &variant.ident) {
                    return Err(syn::Error::new(
                        variant.ident.span(),
                        format!(
                            "`{other}` and `{}` are both written `{segment}`",
                            variant.ident
                        ),
                    ));
                }
                let ident = &variant.ident;
                let (pattern, fields) = bindings(&variant.fields);
                writes.push(quote! {
                    Self::#ident #pattern => {
                        segments.push(::std::string::String::from(#segment));
                        #(::axionomy::path::Segments::write_segments(#fields, segments);)*
                    }
                });
                let constructor = construct(quote!(Self::#ident), &variant.fields);
                reads.push(quote! { #segment => ::std::result::Result::Ok(#constructor), });
            }
            let write = if writes.is_empty() {
                quote!(match *self {})
            } else {
                quote!(match self { #(#writes)* })
            };
            let read = quote! {
                match segments.segment()? {
                    #(#reads)*
                    other => ::std::result::Result::Err(
                        ::axionomy::path::IdentifierError::Unexpected(::std::string::String::from(other)),
                    ),
                }
            };
            (write, read)
        }
        Data::Struct(data) => {
            if data.fields.is_empty() {
                let segment = kebab(name);
                let constructor = construct(quote!(Self), &data.fields);
                (
                    quote!(segments.push(::std::string::String::from(#segment));),
                    quote! {
                        match segments.segment()? {
                            #segment => ::std::result::Result::Ok(#constructor),
                            other => ::std::result::Result::Err(
                                ::axionomy::path::IdentifierError::Unexpected(::std::string::String::from(other)),
                            ),
                        }
                    },
                )
            } else {
                let (pattern, fields) = bindings(&data.fields);
                let constructor = construct(quote!(Self), &data.fields);
                (
                    quote! {
                        let Self #pattern = self;
                        #(::axionomy::path::Segments::write_segments(#fields, segments);)*
                    },
                    quote!(::std::result::Result::Ok(#constructor)),
                )
            }
        }
        Data::Union(data) => {
            return Err(syn::Error::new(
                data.union_token.span,
                "an identifier is an enum or a struct",
            ));
        }
    };
    let segments = quote! {
        impl ::axionomy::path::Segments for #name {
            fn write_segments(&self, segments: &mut ::std::vec::Vec<::std::string::String>) {
                #write
            }

            fn read_segments(
                segments: &mut ::axionomy::path::SegmentReader<'_>,
            ) -> ::std::result::Result<Self, ::axionomy::path::IdentifierError> {
                #read
            }
        }
    };
    let patterns = patterns(input);
    if whole == Whole::Segments {
        return Ok(quote! {
            #segments

            #patterns
        });
    }
    let order = ordering(input)?;
    Ok(quote! {
        #order

        #segments

        #patterns

        impl ::axionomy::__serde::Serialize for #name {
            fn serialize<S: ::axionomy::__serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::std::result::Result<S::Ok, S::Error> {
                ::axionomy::path::serialize_identifier(self, serializer)
            }
        }

        impl<'de> ::axionomy::__serde::Deserialize<'de> for #name {
            fn deserialize<D: ::axionomy::__serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::std::result::Result<Self, D::Error> {
                ::axionomy::path::deserialize_identifier(deserializer)
            }
        }
    })
}

/// The type's typed patterns: `NamePattern`, its mirror with each field a
/// `Slot` of the field's type, written as the type writes a value; and, for
/// an enum whose variants have no fields, a variable's values.
fn patterns(input: &DeriveInput) -> Tokens {
    let name = &input.ident;
    let visibility = &input.vis;
    let mirror = format_ident!("{name}Pattern");
    let slot = |fields: &Fields| -> Tokens {
        let slot_of = |ty: &syn::Type| quote!(::axionomy::pattern::Slot<#ty>);
        match fields {
            Fields::Unit => quote!(),
            Fields::Unnamed(unnamed) => {
                let fields = unnamed.unnamed.iter().map(|field| {
                    let vis = &field.vis;
                    let ty = slot_of(&field.ty);
                    quote!(#vis #ty)
                });
                quote!((#(#fields),*))
            }
            Fields::Named(named) => {
                let fields = named.named.iter().map(|field| {
                    let vis = &field.vis;
                    let ident = &field.ident;
                    let ty = slot_of(&field.ty);
                    quote!(#vis #ident: #ty)
                });
                quote!({ #(#fields),* })
            }
        }
    };
    let segment = |text: &str| {
        quote! {
            pieces.push(::axionomy::pattern::Piece::Segment(
                #text.parse().expect("a variant's name is a segment"),
            ));
        }
    };
    let doc = format!(
        "A pattern of [`{name}`]: each field a value, a variable, or a pattern of its own type."
    );
    let (declaration, write) = match &input.data {
        Data::Enum(data) => {
            let variants = data.variants.iter().map(|variant| {
                let ident = &variant.ident;
                let fields = slot(&variant.fields);
                quote!(#ident #fields)
            });
            let writes = data.variants.iter().map(|variant| {
                let ident = &variant.ident;
                let (pattern, fields) = bindings(&variant.fields);
                let name = segment(&kebab(ident));
                quote! {
                    #mirror::#ident #pattern => {
                        #name
                        #(::axionomy::pattern::Slot::write(#fields, pieces);)*
                    }
                }
            });
            (
                quote! {
                    #[doc = #doc]
                    #[derive(Debug, Clone, PartialEq, Eq)]
                    // Generated beside every identifier, used where rates name it.
                    #[allow(dead_code)]
                    #visibility enum #mirror { #(#variants),* }
                },
                quote!(match pattern { #(#writes)* }),
            )
        }
        Data::Struct(data) => {
            let fields = slot(&data.fields);
            let terminator = if matches!(data.fields, Fields::Named(_)) {
                quote!()
            } else {
                quote!(;)
            };
            let (pattern, bound) = bindings(&data.fields);
            let write = if data.fields.is_empty() {
                segment(&kebab(name))
            } else {
                quote! {
                    let #mirror #pattern = pattern;
                    #(::axionomy::pattern::Slot::write(#bound, pieces);)*
                }
            };
            (
                quote! {
                    #[doc = #doc]
                    #[derive(Debug, Clone, PartialEq, Eq)]
                    // Generated beside every identifier, used where rates name it.
                    #[allow(dead_code)]
                    #visibility struct #mirror #fields #terminator
                },
                write,
            )
        }
        Data::Union(_) => return Tokens::new(),
    };
    let finite = match &input.data {
        Data::Enum(data)
            if !data.variants.is_empty()
                && data
                    .variants
                    .iter()
                    .all(|variant| variant.fields.is_empty()) =>
        {
            let values = data.variants.iter().map(|variant| {
                let ident = &variant.ident;
                quote!(Self::#ident)
            });
            quote! {
                impl ::axionomy::pattern::OneSegment for #name {}

                impl ::axionomy::pattern::Finite for #name {
                    fn values() -> ::std::vec::Vec<Self> {
                        ::std::vec![#(#values),*]
                    }
                }
            }
        }
        _ => quote!(),
    };
    quote! {
        #declaration

        impl ::axionomy::pattern::Patterned for #name {
            type Pattern = #mirror;

            #[allow(unused_variables)]
            fn write_pattern(
                pattern: &Self::Pattern,
                pieces: &mut ::std::vec::Vec<::axionomy::pattern::Piece>,
            ) {
                #write
            }
        }

        impl ::std::convert::From<#mirror> for ::axionomy::Pattern<#name> {
            fn from(pattern: #mirror) -> Self {
                let mut pieces = ::std::vec::Vec::new();
                <#name as ::axionomy::pattern::Patterned>::write_pattern(&pattern, &mut pieces);
                Self::from_pieces(pieces).expect("a typed pattern writes a value of its type")
            }
        }

        #finite
    }
}

/// `Ord` as the natural order of the paths `Segments` writes: variants by
/// their names' order, then fields in order.
fn ordering(input: &DeriveInput) -> syn::Result<Tokens> {
    let name = &input.ident;
    let body = match &input.data {
        Data::Enum(data) => {
            let mut names: Vec<_> = data
                .variants
                .iter()
                .map(|variant| kebab(&variant.ident))
                .collect();
            names.sort();
            let mut ranks = Vec::new();
            let mut pairs = Vec::new();
            for variant in &data.variants {
                let ident = &variant.ident;
                let rank = names
                    .iter()
                    .position(|name| *name == kebab(ident))
                    .expect("every variant has a name") as u32;
                ranks.push(quote!(#name::#ident { .. } => #rank,));
                let (left, right, compare) = compared(&variant.fields);
                pairs.push(quote!((Self::#ident #left, Self::#ident #right) => #compare,));
            }
            if ranks.is_empty() {
                quote!(match *self {})
            } else {
                quote! {
                    fn rank(value: &#name) -> u32 {
                        match value { #(#ranks)* }
                    }
                    match (self, other) {
                        #(#pairs)*
                        _ => ::std::cmp::Ord::cmp(&rank(self), &rank(other)),
                    }
                }
            }
        }
        Data::Struct(data) => {
            let (left, right, compare) = compared(&data.fields);
            quote! {
                let Self #left = self;
                let Self #right = other;
                #compare
            }
        }
        Data::Union(_) => return Ok(Tokens::new()),
    };
    Ok(quote! {
        impl ::std::cmp::Ord for #name {
            fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
                #body
            }
        }

        impl ::std::cmp::PartialOrd for #name {
            fn partial_cmp(&self, other: &Self) -> ::std::option::Option<::std::cmp::Ordering> {
                ::std::option::Option::Some(::std::cmp::Ord::cmp(self, other))
            }
        }
    })
}

/// Patterns binding two values' fields, and their comparison field by field.
fn compared(fields: &Fields) -> (Tokens, Tokens, Tokens) {
    let (names, left, right): (Vec<_>, Vec<_>, Vec<_>) = match fields {
        Fields::Unit => (Vec::new(), Vec::new(), Vec::new()),
        Fields::Unnamed(unnamed) => (0..unnamed.unnamed.len())
            .map(|index| {
                (
                    None,
                    format_ident!("left_{index}"),
                    format_ident!("right_{index}"),
                )
            })
            .fold(
                (Vec::new(), Vec::new(), Vec::new()),
                |mut all, (name, left, right)| {
                    all.0.push(name);
                    all.1.push(left);
                    all.2.push(right);
                    all
                },
            ),
        Fields::Named(named) => named
            .named
            .iter()
            .map(|field| {
                let ident = field.ident.clone().expect("named fields have names");
                (
                    Some(ident.clone()),
                    format_ident!("left_{ident}"),
                    format_ident!("right_{ident}"),
                )
            })
            .fold(
                (Vec::new(), Vec::new(), Vec::new()),
                |mut all, (name, left, right)| {
                    all.0.push(name);
                    all.1.push(left);
                    all.2.push(right);
                    all
                },
            ),
    };
    let pattern = |bindings: &[Ident]| match fields {
        Fields::Unit => quote!(),
        Fields::Unnamed(_) => quote!((#(#bindings),*)),
        Fields::Named(_) => {
            let names = names.iter().map(|name| name.as_ref().expect("named"));
            quote!({ #(#names: #bindings),* })
        }
    };
    let compare = if left.is_empty() {
        quote!(::std::cmp::Ordering::Equal)
    } else {
        quote!(::std::cmp::Ordering::Equal #(.then_with(|| ::std::cmp::Ord::cmp(#left, #right)))*)
    };
    (pattern(&left), pattern(&right), compare)
}

/// The pattern that binds a variant's or struct's fields, and the bindings.
fn bindings(fields: &Fields) -> (Tokens, Vec<Ident>) {
    match fields {
        Fields::Unit => (quote!(), Vec::new()),
        Fields::Unnamed(unnamed) => {
            let names: Vec<_> = (0..unnamed.unnamed.len())
                .map(|index| format_ident!("field_{index}"))
                .collect();
            (quote!((#(#names),*)), names)
        }
        Fields::Named(named) => {
            let names: Vec<_> = named
                .named
                .iter()
                .map(|field| field.ident.clone().expect("named fields have names"))
                .collect();
            (quote!({ #(#names),* }), names)
        }
    }
}

/// `path` built from fields read in order.
fn construct(path: Tokens, fields: &Fields) -> Tokens {
    let read = quote!(::axionomy::path::Segments::read_segments(segments)?);
    match fields {
        Fields::Unit => path,
        Fields::Unnamed(unnamed) => {
            let reads = unnamed.unnamed.iter().map(|_| &read);
            quote!(#path(#(#reads),*))
        }
        Fields::Named(named) => {
            let names = named.named.iter().map(|field| &field.ident);
            quote!(#path { #(#names: #read),* })
        }
    }
}

/// `KeyRoom` → `key-room`, `ABTest` → `ab-test`, `Tick2` → `tick2`.
fn kebab(ident: &Ident) -> String {
    let name = ident.to_string();
    let characters: Vec<char> = name.chars().collect();
    let mut segment = String::with_capacity(name.len() + 4);
    for (index, character) in characters.iter().enumerate() {
        if character.is_ascii_uppercase() && index > 0 {
            let previous = characters[index - 1];
            let next_lower = characters
                .get(index + 1)
                .is_some_and(char::is_ascii_lowercase);
            if previous.is_ascii_lowercase()
                || previous.is_ascii_digit()
                || (previous.is_ascii_uppercase() && next_lower)
            {
                segment.push('-');
            }
        }
        if *character == '_' {
            segment.push('-');
        } else {
            segment.push(character.to_ascii_lowercase());
        }
    }
    segment
}
