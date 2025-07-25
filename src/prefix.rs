#![cfg(feature = "iterator")]
use core::fmt;
use cosmwasm_std::storage_keys::to_length_prefixed_nested;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::marker::PhantomData;

use cosmwasm_std::{Order, Record, StdResult, Storage};
use std::ops::Deref;

use crate::bound::{PrefixBound, RawBound};
use crate::de::KeyDeserialize;
use crate::iter_helpers::{concat, deserialize_kv, deserialize_v, trim};
use crate::keys::Key;
use crate::{Bound, Prefixer, PrimaryKey};

#[derive(Clone)]
pub struct Prefix<K, T, B = Vec<u8>>
where
    K: KeyDeserialize,
    T: Serialize + DeserializeOwned,
{
    /// all namespaces prefixes and concatenated with the key
    pub(crate) storage_prefix: Vec<u8>,
    // see https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-type-parameters for why this is needed
    pub(crate) data: PhantomData<(T, K, B)>,
}

impl<K, T> Debug for Prefix<K, T>
where
    K: KeyDeserialize,
    T: Serialize + DeserializeOwned,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Prefix")
            .field("storage_prefix", &self.storage_prefix)
            .finish_non_exhaustive()
    }
}

impl<K, T> Deref for Prefix<K, T>
where
    K: KeyDeserialize,
    T: Serialize + DeserializeOwned,
{
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.storage_prefix
    }
}

impl<K, T, B> Prefix<K, T, B>
where
    K: KeyDeserialize,
    T: Serialize + DeserializeOwned,
{
    pub fn new(top_name: &[u8], sub_names: &[Key]) -> Self {
        let calculated_len = 1 + sub_names.len();
        let mut combined: Vec<&[u8]> = Vec::with_capacity(calculated_len);
        combined.push(top_name);
        combined.extend(sub_names.iter().map(|sub_name| sub_name.as_ref()));
        debug_assert_eq!(calculated_len, combined.len()); // as long as we calculate correctly, we don't need to reallocate
        let storage_prefix = to_length_prefixed_nested(&combined);
        Prefix {
            storage_prefix,
            data: PhantomData,
        }
    }
}

impl<'b, K, T, B> Prefix<K, T, B>
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
        let mapped = range_with_prefix(
            store,
            &self.storage_prefix,
            min.map(|b| b.to_raw_bound()),
            max.map(|b| b.to_raw_bound()),
            order,
        )
        .map(deserialize_v);
        Box::new(mapped)
    }

    pub fn keys_raw<'a>(
        &self,
        store: &'a dyn Storage,
        min: Option<Bound<'b, B>>,
        max: Option<Bound<'b, B>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = Vec<u8>> + 'a> {
        keys_with_prefix(
            store,
            &self.storage_prefix,
            min.map(|b| b.to_raw_bound()),
            max.map(|b| b.to_raw_bound()),
            order,
        )
    }

    /// Clears the prefix, removing the first `limit` elements (or all if `limit == None`).
    pub fn clear(&self, store: &mut dyn Storage, limit: Option<usize>) {
        const TAKE: usize = 10;
        let mut cleared = false;

        let mut left_to_clear = limit.unwrap_or(usize::MAX);

        while !cleared {
            // Take just TAKE elements to prevent possible heap overflow if the prefix is big,
            // but don't take more than we want to clear.
            let take = TAKE.min(left_to_clear);

            let paths = keys_full(store, &self.storage_prefix, None, None, Order::Ascending)
                .take(take)
                .collect::<Vec<_>>();

            for path in &paths {
                store.remove(path);
            }
            left_to_clear -= paths.len();

            cleared = paths.len() < take || left_to_clear == 0;
        }
    }

    /// Returns `true` if the prefix is empty.
    pub fn is_empty(&self, store: &dyn Storage) -> bool {
        keys_full(store, &self.storage_prefix, None, None, Order::Ascending)
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
        let mapped = range_with_prefix(
            store,
            &self.storage_prefix,
            min.map(|b| b.to_raw_bound()),
            max.map(|b| b.to_raw_bound()),
            order,
        )
        .map(|kv| deserialize_kv::<K, T>(kv));
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
        let mapped = keys_with_prefix(
            store,
            &self.storage_prefix,
            min.map(|b| b.to_raw_bound()),
            max.map(|b| b.to_raw_bound()),
            order,
        )
        .map(|k| K::from_vec(k));
        Box::new(mapped)
    }
}

/// Returns an iterator through all records in storage with the given prefix and
/// within the given bounds, yielding the key without prefix and value.
pub fn range_with_prefix<'a>(
    storage: &'a dyn Storage,
    namespace: &[u8],
    start: Option<RawBound>,
    end: Option<RawBound>,
    order: Order,
) -> Box<dyn Iterator<Item = Record> + 'a> {
    // make a copy for the closure to handle lifetimes safely
    let prefix = namespace.to_vec();
    let mapped =
        range_full(storage, namespace, start, end, order).map(move |(k, v)| (trim(&prefix, &k), v));
    Box::new(mapped)
}

/// Returns an iterator through all keys in storage with the given prefix and
/// within the given bounds, yielding the key without the prefix.
pub fn keys_with_prefix<'a>(
    storage: &'a dyn Storage,
    namespace: &[u8],
    start: Option<RawBound>,
    end: Option<RawBound>,
    order: Order,
) -> Box<dyn Iterator<Item = Vec<u8>> + 'a> {
    // make a copy for the closure to handle lifetimes safely
    let prefix = namespace.to_vec();
    let mapped = keys_full(storage, namespace, start, end, order).map(move |k| trim(&prefix, &k));
    Box::new(mapped)
}

/// Returns an iterator through all records in storage within the given bounds,
/// yielding the full key (including the prefix) and value.
pub(crate) fn range_full<'a>(
    store: &'a dyn Storage,
    namespace: &[u8],
    start: Option<RawBound>,
    end: Option<RawBound>,
    order: Order,
) -> impl Iterator<Item = Record> + 'a {
    let start = calc_start_bound(namespace, start);
    let end = calc_end_bound(namespace, end);

    // get iterator from storage
    store.range(Some(&start), Some(&end), order)
}

/// Returns an iterator through all keys in storage within the given bounds,
/// yielding the full key including the prefix.
pub(crate) fn keys_full<'a>(
    store: &'a dyn Storage,
    namespace: &[u8],
    start: Option<RawBound>,
    end: Option<RawBound>,
    order: Order,
) -> impl Iterator<Item = Vec<u8>> + 'a {
    let start = calc_start_bound(namespace, start);
    let end = calc_end_bound(namespace, end);

    // get iterator from storage
    store.range_keys(Some(&start), Some(&end), order)
}

fn calc_start_bound(namespace: &[u8], bound: Option<RawBound>) -> Vec<u8> {
    match bound {
        None => namespace.to_vec(),
        // this is the natural limits of the underlying Storage
        Some(RawBound::Inclusive(limit)) => concat(namespace, &limit),
        Some(RawBound::Exclusive(limit)) => concat(namespace, &extend_one_byte(&limit)),
    }
}

fn calc_end_bound(namespace: &[u8], bound: Option<RawBound>) -> Vec<u8> {
    match bound {
        None => increment_last_byte(namespace),
        // this is the natural limits of the underlying Storage
        Some(RawBound::Exclusive(limit)) => concat(namespace, &limit),
        Some(RawBound::Inclusive(limit)) => concat(namespace, &extend_one_byte(&limit)),
    }
}

pub fn namespaced_prefix_range<'a, 'c, K: Prefixer<'a>>(
    storage: &'c dyn Storage,
    namespace: &[u8],
    start: Option<PrefixBound<'a, K>>,
    end: Option<PrefixBound<'a, K>>,
    order: Order,
) -> Box<dyn Iterator<Item = Record> + 'c> {
    let prefix = to_length_prefixed_nested(&[namespace]);
    let start = calc_prefix_start_bound(&prefix, start);
    let end = calc_prefix_end_bound(&prefix, end);

    // get iterator from storage
    let base_iterator = storage.range(Some(&start), Some(&end), order);

    // make a copy for the closure to handle lifetimes safely
    let mapped = base_iterator.map(move |(k, v)| (trim(&prefix, &k), v));
    Box::new(mapped)
}

fn calc_prefix_start_bound<'a, K: Prefixer<'a>>(
    namespace: &[u8],
    bound: Option<PrefixBound<'a, K>>,
) -> Vec<u8> {
    match bound.map(|b| b.to_raw_bound()) {
        None => namespace.to_vec(),
        // this is the natural limits of the underlying Storage
        Some(RawBound::Inclusive(limit)) => concat(namespace, &limit),
        Some(RawBound::Exclusive(limit)) => concat(namespace, &increment_last_byte(&limit)),
    }
}

fn calc_prefix_end_bound<'a, K: Prefixer<'a>>(
    namespace: &[u8],
    bound: Option<PrefixBound<'a, K>>,
) -> Vec<u8> {
    match bound.map(|b| b.to_raw_bound()) {
        None => increment_last_byte(namespace),
        // this is the natural limits of the underlying Storage
        Some(RawBound::Exclusive(limit)) => concat(namespace, &limit),
        Some(RawBound::Inclusive(limit)) => concat(namespace, &increment_last_byte(&limit)),
    }
}

pub(crate) fn extend_one_byte(limit: &[u8]) -> Vec<u8> {
    let mut v = limit.to_vec();
    v.push(0);
    v
}

/// Returns a new vec of same length and last byte incremented by one
/// If last bytes are 255, we handle overflow up the chain.
/// If all bytes are 255, this returns wrong data - but that is never possible as a namespace
fn increment_last_byte(input: &[u8]) -> Vec<u8> {
    let mut copy = input.to_vec();
    // zero out all trailing 255, increment first that is not such
    for i in (0..input.len()).rev() {
        if copy[i] == 255 {
            copy[i] = 0;
        } else {
            copy[i] += 1;
            break;
        }
    }
    copy
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::testing::MockStorage;

    #[test]
    fn ensure_proper_range_bounds() {
        let mut store = MockStorage::new();
        // manually create this - not testing nested prefixes here
        let prefix: Prefix<Vec<u8>, u64> = Prefix {
            storage_prefix: b"foo".to_vec(),
            data: PhantomData,
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
        let prefix: Prefix<String, String> = Prefix::new(b"lol", &[Key::Val8([8; 1])]);
        assert_eq!(
            format!("{prefix:?}"),
            "Prefix { storage_prefix: [0, 3, 108, 111, 108, 0, 1, 8], .. }"
        );
    }

    #[test]
    fn prefix_clear_limited() {
        let mut store = MockStorage::new();
        // manually create this - not testing nested prefixes here
        let prefix: Prefix<Vec<u8>, u64> = Prefix {
            storage_prefix: b"foo".to_vec(),
            data: PhantomData,
        };

        // set some data, we care about "foo" prefix
        for i in 0..100u32 {
            store.set(format!("foo{i}").as_bytes(), b"1");
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
        let prefix: Prefix<Vec<u8>, u64> = Prefix {
            storage_prefix: b"foo".to_vec(),
            data: PhantomData,
        };

        // set some data, we care about "foo" prefix
        for i in 0..1000u32 {
            store.set(format!("foo{i}").as_bytes(), b"1");
        }

        // clearing all should work
        prefix.clear(&mut store, None);
        assert_eq!(
            prefix.range(&store, None, None, Order::Ascending).count(),
            0
        );

        // set less data
        for i in 0..5u32 {
            store.set(format!("foo{i}").as_bytes(), b"1");
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
        let prefix: Prefix<Vec<u8>, u64> = Prefix {
            storage_prefix: b"foo".to_vec(),
            data: PhantomData,
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
        let prefix: Prefix<Vec<u8>, u64> = Prefix {
            storage_prefix: b"foo".to_vec(),
            data: PhantomData,
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
