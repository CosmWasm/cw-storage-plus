/*!
Procedural macros helper for interacting with cw-storage-plus and cosmwasm-storage.

For more information on this package, please check out the
[README](https://github.com/CosmWasm/cw-plus/blob/main/packages/storage-macro/README.md).
*/

use proc_macro::TokenStream;
use syn::{
    Ident, Type, TypePath,
    __private::{quote::quote, Span},
    parse_macro_input, ItemStruct, Lifetime,
};

#[proc_macro_attribute]
pub fn index_list(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);

    let ty = Ident::new(&attr.to_string(), Span::call_site());
    let struct_ty = input.ident.clone();

    let names = input
        .fields
        .clone()
        .into_iter()
        .map(|e| {
            let name = e.ident.unwrap();
            quote! { &self.#name }
        })
        .collect::<Vec<_>>();

    let mut first_ix_type = input.fields.iter().next().unwrap().ty.clone();
    // change all lifetime params to 'static
    if let Type::Path(TypePath { ref mut path, .. }) = first_ix_type {
        path.segments.iter_mut().for_each(|seg| {
            if let syn::PathArguments::AngleBracketed(ref mut args) = seg.arguments {
                for arg in args.args.iter_mut() {
                    if let syn::GenericArgument::Lifetime(ref mut lt) = arg {
                        *lt = Lifetime::new("'static", Span::call_site());
                    }
                }
            }
        });
    }

    let expanded = quote! {
        #input

        impl cw_storage_plus::IndexList<#ty> for #struct_ty<'_> {
            type PK = <#first_ix_type as ::cw_storage_plus::Index<#ty>>::PK;

            fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn cw_storage_plus::Index<#ty, PK = Self::PK>> + '_> {
                let v: Vec<&dyn cw_storage_plus::Index<#ty, PK = Self::PK>> = vec![#(#names),*];
                Box::new(v.into_iter())
            }
        }
    };

    TokenStream::from(expanded)
}
