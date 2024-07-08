/* 

##Overview 
 
cw-storage-plus is a refined iteration of cosmwasm-storage, born out of the need for
a more flexible and efficient ORM layer. It transcends the basic KV-store 
abstractions, offering sophisticated data handling through complex key types and generics.
This crate has evolved through multiple releases, incorporating user feedback and practical 
insights to fully leverage Rust's generics. 

##Key Features 

The main goal of cw-storage-plus is to make it easier to build robust contracts by providing:

*Advanced Key Handling: Utilizes complex key types for efficient data access and manipulation. 

*ORM Capabilities: Provides an Object-Relational Mapping layer to work with stored data more intuitively. 

*Generics Utilization: Leverages the power of Rust's generics for versatile and type-safe storage solutions. 

For more information on this package, please check out the
[README](https://github.com/CosmWasm/cw-plus/blob/main/packages/storage-plus/README.md).
*/

mod bound;
mod de;
mod deque;
mod endian;
mod helpers;
mod indexed_map;
mod indexed_snapshot;
mod indexes;
mod int_key;
mod item;
mod iter_helpers;
mod keys;
mod map;
mod namespace;
mod path;
mod prefix;
mod snapshot;

#[cfg(feature = "iterator")]
pub use bound::{Bound, Bounder, PrefixBound, RawBound};
pub use de::KeyDeserialize;
pub use deque::Deque;
pub use deque::DequeIter;
pub use endian::Endian;
#[cfg(feature = "iterator")]
pub use indexed_map::{IndexList, IndexedMap};
#[cfg(feature = "iterator")]
pub use indexed_snapshot::IndexedSnapshotMap;
#[cfg(feature = "iterator")]
pub use indexes::{Index, IndexPrefix, MultiIndex, UniqueIndex};
pub use int_key::IntKey;
pub use item::Item;
pub use keys::{Key, Prefixer, PrimaryKey};
pub use map::Map;
pub use namespace::Namespace;
pub use path::Path;
#[cfg(feature = "iterator")]
pub use prefix::{range_with_prefix, Prefix};
#[cfg(feature = "iterator")]
pub use snapshot::{SnapshotItem, SnapshotMap, Strategy};

// cw_storage_macro reexports
#[cfg(all(feature = "iterator", feature = "macro"))]
#[macro_use]
extern crate cw_storage_macro;
#[cfg(all(feature = "iterator", feature = "macro"))]
/// Auto generate an `IndexList` impl for your indexes struct.
///
/// # Example
///
/// ```rust
/// use cosmwasm_std::Addr;
/// use cw_storage_plus::{MultiIndex, UniqueIndex, index_list};
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
/// struct TestStruct {
///     id: u64,
///     id2: u32,
///     addr: Addr,
/// }
///
/// #[index_list(TestStruct)] // <- Add this line right here.
/// struct TestIndexes<'a> {
///     id: MultiIndex<'a, u32, TestStruct, u64>,
///     addr: UniqueIndex<'a, Addr, TestStruct, ()>,
/// }
/// ```
///
pub use cw_storage_macro::index_list;
