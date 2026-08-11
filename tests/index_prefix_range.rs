//! Regression tests for `MultiIndex::prefix_range` / `UniqueIndex::prefix_range`.
//!
//! Both used to map their results with the generic `deserialize_kv` instead of the
//! index-aware deserializer, so `prefix_range` returned the wrong data (the stored
//! pk-length for `MultiIndex`, or a deserialization error) instead of the value that
//! `range` and `prefix_range_raw` return. These tests pin the two methods to the same
//! values their siblings produce.

#![cfg(feature = "iterator")]

use cosmwasm_std::testing::MockStorage;
use cosmwasm_std::Order;
use cw_storage_plus::{Index, IndexList, IndexedMap, MultiIndex, UniqueIndex};

struct MultiIndexes<'a> {
    // one shared index value so an unbounded prefix_range walks every entry
    by_cat: MultiIndex<'a, u32, u64, String>,
}

impl IndexList<u64> for MultiIndexes<'_> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<u64>> + '_> {
        Box::new(vec![&self.by_cat as &dyn Index<u64>].into_iter())
    }
}

fn multi_map<'a>() -> IndexedMap<&'a str, u64, MultiIndexes<'a>> {
    IndexedMap::new(
        "m",
        MultiIndexes {
            by_cat: MultiIndex::new(|_pk, _v| 0u32, "m", "m__cat"),
        },
    )
}

#[test]
fn multi_index_prefix_range_returns_values_not_pk_lengths() {
    let mut store = MockStorage::new();
    let map = multi_map();
    map.save(&mut store, "a", &111_111).unwrap();
    map.save(&mut store, "b", &222_222).unwrap();
    map.save(&mut store, "c", &333_333).unwrap();

    let by_range: Vec<u64> = map
        .idx
        .by_cat
        .range(&store, None, None, Order::Ascending)
        .map(|r| r.unwrap().1)
        .collect();

    let by_prefix_range: Vec<u64> = map
        .idx
        .by_cat
        .prefix_range(&store, None, None, Order::Ascending)
        .map(|r| r.unwrap().1)
        .collect();

    assert_eq!(by_range, vec![111_111, 222_222, 333_333]);
    assert_eq!(
        by_prefix_range, by_range,
        "prefix_range must return the stored values, matching range()"
    );
}

struct UniqueIndexes<'a> {
    by_val: UniqueIndex<'a, u32, u64, String>,
}

impl IndexList<u64> for UniqueIndexes<'_> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<u64>> + '_> {
        Box::new(vec![&self.by_val as &dyn Index<u64>].into_iter())
    }
}

fn unique_map<'a>() -> IndexedMap<&'a str, u64, UniqueIndexes<'a>> {
    IndexedMap::new(
        "u",
        UniqueIndexes {
            by_val: UniqueIndex::new(|v| *v as u32, "u__val"),
        },
    )
}

#[test]
fn unique_index_prefix_range_returns_values() {
    let mut store = MockStorage::new();
    let map = unique_map();
    map.save(&mut store, "a", &10).unwrap();
    map.save(&mut store, "b", &20).unwrap();

    let by_range: Vec<u64> = map
        .idx
        .by_val
        .range(&store, None, None, Order::Ascending)
        .map(|r| r.unwrap().1)
        .collect();

    let by_prefix_range: Vec<u64> = map
        .idx
        .by_val
        .prefix_range(&store, None, None, Order::Ascending)
        .map(|r| r.unwrap().1)
        .collect();

    assert_eq!(by_range, vec![10, 20]);
    assert_eq!(
        by_prefix_range, by_range,
        "UniqueIndex::prefix_range must return the stored values, matching range()"
    );
}
