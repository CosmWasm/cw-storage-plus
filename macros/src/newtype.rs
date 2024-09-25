use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

pub fn cw_storage_newtype_key_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let expanded = impl_newtype(&input);

    TokenStream::from(expanded)
}

fn impl_newtype(input: &DeriveInput) -> proc_macro2::TokenStream {
    // Extract the struct name
    let name = &input.ident;

    // Extract the inner type
    let inner_type = match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Some(&fields.unnamed[0].ty),
            _ => None,
        },
        _ => None,
    };

    let inner_type = inner_type
        .expect("NewTypeKey can only be derived for newtypes (tuple structs with one field)");

    // Implement PrimaryKey
    let impl_primary_key = quote! {
        impl<'a> cw_storage_plus::PrimaryKey<'a> for #name
        where
            #inner_type: cw_storage_plus::PrimaryKey<'a>,
        {
            type Prefix = ();
            type SubPrefix = ();
            type Suffix = Self;
            type SuperSuffix = Self;

            fn key(&self) -> Vec<cw_storage_plus::Key> {
                self.0.key()
            }
        }
    };

    // Implement Prefixer
    let impl_prefixer = quote! {
        impl<'a> cw_storage_plus::Prefixer<'a> for #name
        where
            #inner_type: cw_storage_plus::Prefixer<'a>,
        {
            fn prefix(&self) -> Vec<cw_storage_plus::Key> {
                self.0.prefix()
            }
        }
    };

    // Implement KeyDeserialize
    let impl_key_deserialize = quote! {
        impl cw_storage_plus::KeyDeserialize for #name
        where
            #inner_type: cw_storage_plus::KeyDeserialize<Output = #inner_type>,
        {
            type Output = #name;
            const KEY_ELEMS: u16 = 1;

            #[inline(always)]
            fn from_vec(value: Vec<u8>) -> cosmwasm_std::StdResult<Self::Output> {
                <#inner_type as cw_storage_plus::KeyDeserialize>::from_vec(value).map(#name)
            }
        }
    };

    // Combine all implementations
    let expanded = quote! {
        #impl_primary_key
        #impl_prefixer
        #impl_key_deserialize
    };

    expanded
}
