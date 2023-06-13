# Migrating

This guide lists API changes between *cw-storage-plus* major releases.

## v1.0.x -> v2.0.0

### Breaking Issues / PRs

- The `UniqueIndex` `PK` trait parameter is now mandatory [\#37](https://github.com/CosmWasm/cw-storage-plus/issues/37).

  The migration is straightforward: just add the `PK` type parameter to your `UniqueIndex` implementation. If you don't
  plan to deserialize it, you can use `()` as your `UniqueIndex` `PK` type, which was the default before.

- The `KeyDeserialize` trait now includes a `KEY_ELEMS` const [\#34](https://github.com/CosmWasm/cw-storage-plus/pull/34),
  that needs to be defined when implementing this trait. This const defines the number of elements in the key, and its
  value would typically be `1`.

  This only affect users that implement `KeyDeserialize` for their own types. If you only use the provided types, you
  don't need to worry about this.
