#![cfg(feature = "iterator")]
use core::fmt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;

use cosmwasm_std::{Order, Record, StdResult, Storage};
use std::ops::Deref;

use crate::de::KeyDeserialize;
use crate::iter_helpers::{deserialize_kv, deserialize_v};
use crate::keys::Key;
use crate::{Bound, PrimaryKey};

type DeserializeVFn<T> = fn(&dyn Storage, &[u8], Record) -> StdResult<Record<T>>;

type DeserializeKvFn<K, T> =
    fn(&dyn Storage, &[u8], Record) -> StdResult<(<K as KeyDeserialize>::Output, T)>;

pub fn default_deserializer_v<T: DeserializeOwned>(
    _: &dyn Storage,
    _: &[u8],
    raw: Record,
) -> StdResult<Record<T>> {
    deserialize_v(raw)
}

pub fn default_deserializer_kv<K: KeyDeserialize, T: DeserializeOwned>(
    _: &dyn Storage,
    _: &[u8],
    raw: Record,
) -> StdResult<(K::Output, T)> {
    deserialize_kv::<K, T>(raw)
}

#[derive(Clone)]
pub struct IndexPrefix<K, T, B = Vec<u8>>
where
    K: KeyDeserialize,
    T: Serialize + DeserializeOwned,
{
    inner: crate::prefix::Prefix<K, T, B>,
    pk_name: Vec<u8>,
    de_fn_kv: DeserializeKvFn<K, T>,
    de_fn_v: DeserializeVFn<T>,
}

impl<K, T> Debug for IndexPrefix<K, T>
where
    K: KeyDeserialize,
    T: Serialize + DeserializeOwned,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IndexPrefix")
            .field("storage_prefix", &self.inner.storage_prefix)
            .field("pk_name", &self.pk_name)
            .finish_non_exhaustive()
    }
}

impl<K, T> Deref for IndexPrefix<K, T>
where
    K: KeyDeserialize,
    T: Serialize + DeserializeOwned,
{
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.inner.storage_prefix
    }
}

impl<K, T, B> IndexPrefix<K, T, B>
where
    K: KeyDeserialize,
    T: Serialize + DeserializeOwned,
{
    pub fn new(top_name: &[u8], sub_names: &[Key]) -> Self {
        IndexPrefix::with_deserialization_functions(
            top_name,
            sub_names,
            &[],
            default_deserializer_kv::<K, T>,
            default_deserializer_v,
        )
    }

    pub fn with_deserialization_functions(
        top_name: &[u8],
        sub_names: &[Key],
        pk_name: &[u8],
        de_fn_kv: DeserializeKvFn<K, T>,
        de_fn_v: DeserializeVFn<T>,
    ) -> Self {
        IndexPrefix {
            inner: crate::prefix::Prefix::new(top_name, sub_names),
            pk_name: pk_name.to_vec(),
            de_fn_kv,
            de_fn_v,
        }
    }
}

impl<'b, K, T, B> IndexPrefix<K, T, B>
where
    B: PrimaryKey<'b>,
    K: KeyDeserialize,
    T: Serialize + DeserializeOwned,
{
    pub fn range_raw<'a>(
        &self,
        store: &'a dyn Storage,
        min: Option<Bound<'b, B>>,
        max: Option<Bound<'b, B>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<Record<T>>> + 'a>
    where
        T: 'a,
    {
        let de_fn = self.de_fn_v;
        let pk_name = self.pk_name.clone();
        let mapped = crate::prefix::range_with_prefix(
            store,
            &self.inner.storage_prefix,
            min.map(|b| b.to_raw_bound()),
            max.map(|b| b.to_raw_bound()),
            order,
        )
        .map(move |kv| (de_fn)(store, &pk_name, kv));
        Box::new(mapped)
    }

    pub fn keys_raw<'a>(
        &self,
        store: &'a dyn Storage,
        min: Option<Bound<'b, B>>,
        max: Option<Bound<'b, B>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = Vec<u8>> + 'a> {
        crate::prefix::keys_with_prefix(
            store,
            &self.inner.storage_prefix,
            min.map(|b| b.to_raw_bound()),
            max.map(|b| b.to_raw_bound()),
            order,
        )
    }

    /// Clears the prefix, removing the first `limit` elements (or all if `limit == None`).
    pub fn clear(&self, store: &mut dyn Storage, limit: Option<usize>) {
        self.inner.clear(store, limit);
    }

    /// Returns `true` if the prefix is empty.
    pub fn is_empty(&self, store: &dyn Storage) -> bool {
        crate::prefix::keys_full(
            store,
            &self.inner.storage_prefix,
            None,
            None,
            Order::Ascending,
        )
        .next()
        .is_none()
    }

    pub fn range<'a>(
        &self,
        store: &'a dyn Storage,
        min: Option<Bound<'b, B>>,
        max: Option<Bound<'b, B>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<(K::Output, T)>> + 'a>
    where
        T: 'a,
        K::Output: 'static,
    {
        let de_fn = self.de_fn_kv;
        let pk_name = self.pk_name.clone();
        let mapped = crate::prefix::range_with_prefix(
            store,
            &self.inner.storage_prefix,
            min.map(|b| b.to_raw_bound()),
            max.map(|b| b.to_raw_bound()),
            order,
        )
        .map(move |kv| (de_fn)(store, &pk_name, kv));
        Box::new(mapped)
    }

    pub fn keys<'a>(
        &self,
        store: &'a dyn Storage,
        min: Option<Bound<'b, B>>,
        max: Option<Bound<'b, B>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<K::Output>> + 'a>
    where
        T: 'a,
        K::Output: 'static,
    {
        let de_fn = self.de_fn_kv;
        let pk_name = self.pk_name.clone();
        let mapped = crate::prefix::range_with_prefix(
            store,
            &self.inner.storage_prefix,
            min.map(|b| b.to_raw_bound()),
            max.map(|b| b.to_raw_bound()),
            order,
        )
        .map(move |kv| (de_fn)(store, &pk_name, kv).map(|(k, _)| k));
        Box::new(mapped)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::marker::PhantomData;

    use cosmwasm_std::testing::MockStorage;

    #[test]
    fn ensure_proper_range_bounds() {
        let mut store = MockStorage::new();
        // manually create this - not testing nested prefixes here
        let prefix: IndexPrefix<Vec<u8>, u64> = IndexPrefix {
            inner: crate::prefix::Prefix {
                storage_prefix: b"foo".to_vec(),
                data: PhantomData::<(u64, _, _)>,
            },
            pk_name: vec![],
            de_fn_kv: |_, _, kv| deserialize_kv::<Vec<u8>, u64>(kv),
            de_fn_v: |_, _, kv| deserialize_v(kv),
        };

        // set some data, we care about "foo" prefix
        store.set(b"foobar", b"1");
        store.set(b"foora", b"2");
        store.set(b"foozi", b"3");
        // these shouldn't match
        store.set(b"foply", b"100");
        store.set(b"font", b"200");

        let expected = vec![
            (b"bar".to_vec(), 1u64),
            (b"ra".to_vec(), 2u64),
            (b"zi".to_vec(), 3u64),
        ];
        let expected_reversed: Vec<(Vec<u8>, u64)> = expected.iter().rev().cloned().collect();

        // let's do the basic sanity check
        let res: StdResult<Vec<_>> = prefix
            .range_raw(&store, None, None, Order::Ascending)
            .collect();
        assert_eq!(&expected, &res.unwrap());
        let res: StdResult<Vec<_>> = prefix
            .range_raw(&store, None, None, Order::Descending)
            .collect();
        assert_eq!(&expected_reversed, &res.unwrap());

        // now let's check some ascending ranges
        let res: StdResult<Vec<_>> = prefix
            .range_raw(
                &store,
                Some(Bound::inclusive(b"ra".to_vec())),
                None,
                Order::Ascending,
            )
            .collect();
        assert_eq!(&expected[1..], res.unwrap().as_slice());
        // skip excluded
        let res: StdResult<Vec<_>> = prefix
            .range_raw(
                &store,
                Some(Bound::exclusive(b"ra".to_vec())),
                None,
                Order::Ascending,
            )
            .collect();
        assert_eq!(&expected[2..], res.unwrap().as_slice());
        // if we exclude something a little lower, we get matched
        let res: StdResult<Vec<_>> = prefix
            .range_raw(
                &store,
                Some(Bound::exclusive(b"r".to_vec())),
                None,
                Order::Ascending,
            )
            .collect();
        assert_eq!(&expected[1..], res.unwrap().as_slice());

        // now let's check some descending ranges
        let res: StdResult<Vec<_>> = prefix
            .range_raw(
                &store,
                None,
                Some(Bound::inclusive(b"ra".to_vec())),
                Order::Descending,
            )
            .collect();
        assert_eq!(&expected_reversed[1..], res.unwrap().as_slice());
        // skip excluded
        let res: StdResult<Vec<_>> = prefix
            .range_raw(
                &store,
                None,
                Some(Bound::exclusive(b"ra".to_vec())),
                Order::Descending,
            )
            .collect();
        assert_eq!(&expected_reversed[2..], res.unwrap().as_slice());
        // if we exclude something a little higher, we get matched
        let res: StdResult<Vec<_>> = prefix
            .range_raw(
                &store,
                None,
                Some(Bound::exclusive(b"rb".to_vec())),
                Order::Descending,
            )
            .collect();
        assert_eq!(&expected_reversed[1..], res.unwrap().as_slice());

        // now test when both sides are set
        let res: StdResult<Vec<_>> = prefix
            .range_raw(
                &store,
                Some(Bound::inclusive(b"ra".to_vec())),
                Some(Bound::exclusive(b"zi".to_vec())),
                Order::Ascending,
            )
            .collect();
        assert_eq!(&expected[1..2], res.unwrap().as_slice());
        // and descending
        let res: StdResult<Vec<_>> = prefix
            .range_raw(
                &store,
                Some(Bound::inclusive(b"ra".to_vec())),
                Some(Bound::exclusive(b"zi".to_vec())),
                Order::Descending,
            )
            .collect();
        assert_eq!(&expected[1..2], res.unwrap().as_slice());
        // Include both sides
        let res: StdResult<Vec<_>> = prefix
            .range_raw(
                &store,
                Some(Bound::inclusive(b"ra".to_vec())),
                Some(Bound::inclusive(b"zi".to_vec())),
                Order::Descending,
            )
            .collect();
        assert_eq!(&expected_reversed[..2], res.unwrap().as_slice());
        // Exclude both sides
        let res: StdResult<Vec<_>> = prefix
            .range_raw(
                &store,
                Some(Bound::exclusive(b"ra".to_vec())),
                Some(Bound::exclusive(b"zi".to_vec())),
                Order::Ascending,
            )
            .collect();
        assert_eq!(res.unwrap().as_slice(), &[]);
    }

    #[test]
    fn prefix_debug() {
        let prefix: IndexPrefix<String, String> = IndexPrefix::new(b"lol", &[Key::Val8([8; 1])]);
        assert_eq!(
            format!("{:?}", prefix),
            "IndexPrefix { storage_prefix: [0, 3, 108, 111, 108, 0, 1, 8], pk_name: [], .. }"
        );
    }

    #[test]
    fn prefix_clear_limited() {
        let mut store = MockStorage::new();
        // manually create this - not testing nested prefixes here
        let prefix: IndexPrefix<Vec<u8>, u64> = IndexPrefix {
            inner: crate::prefix::Prefix {
                storage_prefix: b"foo".to_vec(),
                data: PhantomData::<(u64, _, _)>,
            },
            pk_name: vec![],
            de_fn_kv: |_, _, kv| deserialize_kv::<Vec<u8>, u64>(kv),
            de_fn_v: |_, _, kv| deserialize_v(kv),
        };

        // set some data, we care about "foo" prefix
        for i in 0..100u32 {
            store.set(format!("foo{}", i).as_bytes(), b"1");
        }

        // clearing less than `TAKE` should work
        prefix.clear(&mut store, Some(1));
        assert_eq!(
            prefix.range(&store, None, None, Order::Ascending).count(),
            99
        );

        // clearing more than `TAKE` should work
        prefix.clear(&mut store, Some(12));
        assert_eq!(
            prefix.range(&store, None, None, Order::Ascending).count(),
            99 - 12
        );

        // clearing an exact multiple of `TAKE` should work
        prefix.clear(&mut store, Some(20));
        assert_eq!(
            prefix.range(&store, None, None, Order::Ascending).count(),
            99 - 12 - 20
        );

        // clearing more than available should work
        prefix.clear(&mut store, Some(1000));
        assert_eq!(
            prefix.range(&store, None, None, Order::Ascending).count(),
            0
        );
    }

    #[test]
    fn prefix_clear_unlimited() {
        let mut store = MockStorage::new();
        // manually create this - not testing nested prefixes here
        let prefix: IndexPrefix<Vec<u8>, u64> = IndexPrefix {
            inner: crate::prefix::Prefix {
                storage_prefix: b"foo".to_vec(),
                data: PhantomData::<(u64, _, _)>,
            },
            pk_name: vec![],
            de_fn_kv: |_, _, kv| deserialize_kv::<Vec<u8>, u64>(kv),
            de_fn_v: |_, _, kv| deserialize_v(kv),
        };

        // set some data, we care about "foo" prefix
        for i in 0..1000u32 {
            store.set(format!("foo{}", i).as_bytes(), b"1");
        }

        // clearing all should work
        prefix.clear(&mut store, None);
        assert_eq!(
            prefix.range(&store, None, None, Order::Ascending).count(),
            0
        );

        // set less data
        for i in 0..5u32 {
            store.set(format!("foo{}", i).as_bytes(), b"1");
        }

        // clearing all should work
        prefix.clear(&mut store, None);
        assert_eq!(
            prefix.range(&store, None, None, Order::Ascending).count(),
            0
        );
    }

    #[test]
    fn is_empty_works() {
        // manually create this - not testing nested prefixes here
        let prefix: IndexPrefix<Vec<u8>, u64> = IndexPrefix {
            inner: crate::prefix::Prefix {
                storage_prefix: b"foo".to_vec(),
                data: PhantomData::<(u64, _, _)>,
            },
            pk_name: vec![],
            de_fn_kv: |_, _, kv| deserialize_kv::<Vec<u8>, u64>(kv),
            de_fn_v: |_, _, kv| deserialize_v(kv),
        };

        let mut storage = MockStorage::new();

        assert!(prefix.is_empty(&storage));

        storage.set(b"fookey1", b"1");
        storage.set(b"fookey2", b"2");

        assert!(!prefix.is_empty(&storage));
    }

    #[test]
    fn keys_raw_works() {
        // manually create this - not testing nested prefixes here
        let prefix: IndexPrefix<Vec<u8>, u64> = IndexPrefix {
            inner: crate::prefix::Prefix {
                storage_prefix: b"foo".to_vec(),
                data: PhantomData::<(u64, _, _)>,
            },
            pk_name: vec![],
            de_fn_kv: |_, _, kv| deserialize_kv::<Vec<u8>, u64>(kv),
            de_fn_v: |_, _, kv| deserialize_v(kv),
        };

        let mut storage = MockStorage::new();
        storage.set(b"fookey1", b"1");
        storage.set(b"fookey2", b"2");

        let keys: Vec<_> = prefix
            .keys_raw(&storage, None, None, Order::Ascending)
            .collect();
        assert_eq!(keys, vec![b"key1", b"key2"]);

        let keys: Vec<_> = prefix
            .keys_raw(
                &storage,
                Some(Bound::exclusive("key1")),
                None,
                Order::Ascending,
            )
            .collect();
        assert_eq!(keys, vec![b"key2"]);
    }
}
