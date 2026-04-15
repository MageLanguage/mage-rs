use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, parse_macro_input};

fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    for (index, character) in name.chars().enumerate() {
        if character.is_uppercase() {
            if index > 0 {
                result.push('_');
            }
            for lower in character.to_lowercase() {
                result.push(lower);
            }
        } else {
            result.push(character);
        }
    }
    result
}

fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Option";
    }
    false
}

fn generate_variant_arm(
    variant: &syn::Variant,
    enum_ident: &syn::Ident,
) -> proc_macro2::TokenStream {
    let variant_ident = &variant.ident;
    let section_name = to_snake_case(&variant_ident.to_string());

    match &variant.fields {
        Fields::Named(fields) => {
            let field_names: Vec<_> = fields
                .named
                .iter()
                .map(|field| field.ident.as_ref().unwrap())
                .collect();

            let field_types: Vec<_> = fields.named.iter().map(|field| &field.ty).collect();

            let mut regular_fields = Vec::new();
            let mut option_fields = Vec::new();

            for (field_name, field_type) in field_names.iter().zip(field_types.iter()) {
                let key = field_name.to_string();
                if is_option_type(field_type) {
                    option_fields.push((*field_name, key));
                } else {
                    regular_fields.push((*field_name, key));
                }
            }

            let regular_inlines = regular_fields.iter().map(|(name, key)| {
                quote! { .inline(#key, #name) }
            });

            if option_fields.is_empty() {
                quote! {
                    #enum_ident::#variant_ident { #(#field_names),* } => {
                        serde_text::Section::new(#section_name)
                            #(#regular_inlines)*
                    }
                }
            } else {
                let option_statements = option_fields.iter().map(|(name, key)| {
                    quote! {
                        if let Some(value) = #name {
                            section = section.inline(#key, value);
                        }
                    }
                });

                quote! {
                    #enum_ident::#variant_ident { #(#field_names),* } => {
                        let mut section = serde_text::Section::new(#section_name)
                            #(#regular_inlines)*;
                        #(#option_statements)*
                        section
                    }
                }
            }
        }
        Fields::Unit => {
            quote! {
                #enum_ident::#variant_ident => {
                    serde_text::Section::new(#section_name)
                }
            }
        }
        Fields::Unnamed(_) => {
            syn::Error::new_spanned(variant, "ToSection does not support tuple variants")
                .to_compile_error()
        }
    }
}

#[proc_macro_derive(ToSection)]
pub fn derive_to_section(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let Data::Enum(data_enum) = &input.data else {
        return syn::Error::new_spanned(&input, "ToSection can only be derived for enums")
            .to_compile_error()
            .into();
    };

    let match_arms = data_enum
        .variants
        .iter()
        .map(|variant| generate_variant_arm(variant, name));

    let expanded = quote! {
        impl serde_text::ToSection for #name {
            fn to_section(&self) -> serde_text::Section {
                match self {
                    #(#match_arms)*
                }
            }
        }
    };

    expanded.into()
}
