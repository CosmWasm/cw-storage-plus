/*!
Procedural macros helper for interacting with cw-storage-plus and cosmwasm-storage.

For more information on this package, please check out the
[README](https://github.com/CosmWasm/cw-storage-plus/blob/main/macros/README.md).
*/

mod index_list;
mod newtype;

use proc_macro::TokenStream;

// Re-export the procedural macro functions

#[proc_macro_attribute]
pub fn index_list(attr: TokenStream, item: TokenStream) -> TokenStream {
    index_list::index_list(attr, item)
}

#[proc_macro_derive(NewTypeKey)]
pub fn cw_storage_newtype_key_derive(input: TokenStream) -> TokenStream {
    newtype::cw_storage_newtype_key_derive(input)
}
