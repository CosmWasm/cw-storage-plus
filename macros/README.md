# CW-Storage-Plus: Macro helpers for storage-plus 

Procedural macros helper for interacting with cw-storage-plus and cosmwasm-storage.

## Current features

Auto generate an `IndexList` impl for your indexes struct.

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct TestStruct {
    id: u64,
    id2: u32,
    addr: Addr,
}

#[index_list(TestStruct)] // <- Add this line right here.
struct TestIndexes<'a> {
    id: MultiIndex<'a, u32, TestStruct, u64>,
    addr: UniqueIndex<'a, Addr, TestStruct, String>,
}
```

Auto generate the required impls to use a newtype as a key

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[derive(NewTypeKey)] // <- Add this line right here.
struct TestKey(u64);

// You can now use `TestKey` as a key in `Map`
```