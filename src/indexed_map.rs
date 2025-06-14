// this module requires iterator to be useful at all
#![cfg(feature = "iterator")]

use crate::namespace::Namespace;
use crate::PrefixBound;
use cosmwasm_std::{StdError, StdResult, Storage};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::de::KeyDeserialize;
use crate::indexes::Index;
use crate::iter_helpers::{deserialize_kv, deserialize_v};
use crate::keys::{Prefixer, PrimaryKey};
use crate::map::Map;
use crate::prefix::{namespaced_prefix_range, Prefix};
use crate::{Bound, Path};

pub trait IndexList<T> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<T>> + '_>;
}

/// `IndexedMap` works like a `Map` but has a secondary index
pub struct IndexedMap<K, T, I> {
    pk_namespace: Namespace,
    primary: Map<K, T>,
    /// This is meant to be read directly to get the proper types, like:
    /// map.idx.owner.items(...)
    pub idx: I,
}

impl<'a, K, T, I> IndexedMap<K, T, I>
where
    K: PrimaryKey<'a>,
    T: Serialize + DeserializeOwned + Clone,
    I: IndexList<T>,
{
    /// Creates a new [`IndexedMap`] with the given storage key. This is a constant function only suitable
    /// when you have a prefix in the form of a static string slice.
    pub const fn new(pk_namespace: &'static str, indexes: I) -> Self {
        IndexedMap {
            pk_namespace: Namespace::from_static_str(pk_namespace),
            primary: Map::new(pk_namespace),
            idx: indexes,
        }
    }

    /// Creates a new [`IndexedMap`] with the given storage key. Use this if you might need to handle
    /// a dynamic string. Otherwise, you should probably prefer [`IndexedMap::new`].
    pub fn new_dyn(pk_namespace: impl Into<Namespace>, indexes: I) -> Self {
        let pk_namespace = pk_namespace.into();

        IndexedMap {
            pk_namespace: pk_namespace.clone(),
            primary: Map::new_dyn(pk_namespace),
            idx: indexes,
        }
    }

    pub fn key(&self, k: K) -> Path<T> {
        self.primary.key(k)
    }
}

impl<'a, K, T, I> IndexedMap<K, T, I>
where
    K: PrimaryKey<'a>,
    T: Serialize + DeserializeOwned + Clone,
    I: IndexList<T>,
{
    /// save will serialize the model and store, returns an error on serialization issues.
    /// this must load the old value to update the indexes properly
    /// if you loaded the old value earlier in the same function, use replace to avoid needless db reads
    pub fn save(&self, store: &mut dyn Storage, key: K, data: &T) -> StdResult<()> {
        let old_data = self.may_load(store, key.clone())?;
        self.replace(store, key, Some(data), old_data.as_ref())
    }

    pub fn remove(&self, store: &mut dyn Storage, key: K) -> StdResult<()> {
        let old_data = self.may_load(store, key.clone())?;
        self.replace(store, key, None, old_data.as_ref())
    }

    /// replace writes data to key. old_data must be the current stored value (from a previous load)
    /// and is used to properly update the index. This is used by save, replace, and update
    /// and can be called directly if you want to optimize
    pub fn replace(
        &self,
        store: &mut dyn Storage,
        key: K,
        data: Option<&T>,
        old_data: Option<&T>,
    ) -> StdResult<()> {
        // this is the key *relative* to the primary map namespace
        let pk = key.joined_key();
        if let Some(old) = old_data {
            for index in self.idx.get_indexes() {
                index.remove(store, &pk, old)?;
            }
        }
        if let Some(updated) = data {
            for index in self.idx.get_indexes() {
                index.save(store, &pk, updated)?;
            }
            self.primary.save(store, key, updated)?;
        } else {
            self.primary.remove(store, key);
        }
        Ok(())
    }

    /// Loads the data, perform the specified action, and store the result
    /// in the database. This is shorthand for some common sequences, which may be useful.
    ///
    /// If the data exists, `action(Some(value))` is called. Otherwise `action(None)` is called.
    pub fn update<A, E>(&self, store: &mut dyn Storage, key: K, action: A) -> Result<T, E>
    where
        A: FnOnce(Option<T>) -> Result<T, E>,
        E: From<StdError>,
    {
        let input = self.may_load(store, key.clone())?;
        let old_val = input.clone();
        let output = action(input)?;
        self.replace(store, key, Some(&output), old_val.as_ref())?;
        Ok(output)
    }

    // Everything else, that doesn't touch indexers, is just pass-through from self.core,
    // thus can be used from while iterating over indexes

    /// load will return an error if no data is set at the given key, or on parse error
    pub fn load(&self, store: &dyn Storage, key: K) -> StdResult<T> {
        self.primary.load(store, key)
    }

    /// may_load will parse the data stored at the key if present, returns Ok(None) if no data there.
    /// returns an error on issues parsing
    pub fn may_load(&self, store: &dyn Storage, key: K) -> StdResult<Option<T>> {
        self.primary.may_load(store, key)
    }

    /// Returns true if storage contains this key, without parsing or interpreting the contents.
    pub fn has(&self, store: &dyn Storage, k: K) -> bool {
        self.primary.key(k).has(store)
    }

    // use no_prefix to scan -> range
    fn no_prefix_raw(&self) -> Prefix<Vec<u8>, T, K> {
        Prefix::new(self.pk_namespace.as_slice(), &[])
    }

    /// Clears the map, removing all elements.
    pub fn clear(&self, store: &mut dyn Storage) {
        const TAKE: usize = 10;
        let mut cleared = false;

        while !cleared {
            let paths = self
                .no_prefix_raw()
                .keys_raw(store, None, None, cosmwasm_std::Order::Ascending)
                .map(|raw_key| Path::<T>::new(self.pk_namespace.as_slice(), &[raw_key.as_slice()]))
                // Take just TAKE elements to prevent possible heap overflow if the Map is big.
                .take(TAKE)
                .collect::<Vec<_>>();

            paths.iter().for_each(|path| store.remove(path));

            cleared = paths.len() < TAKE;
        }
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self, store: &dyn Storage) -> bool {
        self.no_prefix_raw()
            .keys_raw(store, None, None, cosmwasm_std::Order::Ascending)
            .next()
            .is_none()
    }
}

#[cfg(feature = "iterator")]
impl<'a, K, T, I> IndexedMap<K, T, I>
where
    K: PrimaryKey<'a>,
    T: Serialize + DeserializeOwned + Clone,
    I: IndexList<T>,
{
    /// While `range_raw` over a `prefix` fixes the prefix to one element and iterates over the
    /// remaining, `prefix_range_raw` accepts bounds for the lowest and highest elements of the `Prefix`
    /// itself, and iterates over those (inclusively or exclusively, depending on `PrefixBound`).
    /// There are some issues that distinguish these two, and blindly casting to `Vec<u8>` doesn't
    /// solve them.
    pub fn prefix_range_raw<'c>(
        &self,
        store: &'c dyn Storage,
        min: Option<PrefixBound<'a, K::Prefix>>,
        max: Option<PrefixBound<'a, K::Prefix>>,
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = StdResult<cosmwasm_std::Record<T>>> + 'c>
    where
        T: 'c,
        'a: 'c,
    {
        let mapped = namespaced_prefix_range(store, self.pk_namespace.as_slice(), min, max, order)
            .map(deserialize_v);
        Box::new(mapped)
    }
}

#[cfg(feature = "iterator")]
impl<'a, K, T, I> IndexedMap<K, T, I>
where
    T: Serialize + DeserializeOwned + Clone,
    K: PrimaryKey<'a>,
    I: IndexList<T>,
{
    pub fn sub_prefix(&self, p: K::SubPrefix) -> Prefix<K::SuperSuffix, T, K::SuperSuffix> {
        Prefix::new(self.pk_namespace.as_slice(), &p.prefix())
    }

    pub fn prefix(&self, p: K::Prefix) -> Prefix<K::Suffix, T, K::Suffix> {
        Prefix::new(self.pk_namespace.as_slice(), &p.prefix())
    }
}

#[cfg(feature = "iterator")]
impl<'a, K, T, I> IndexedMap<K, T, I>
where
    T: Serialize + DeserializeOwned + Clone,
    K: PrimaryKey<'a> + KeyDeserialize,
    I: IndexList<T>,
{
    /// While `range` over a `prefix` fixes the prefix to one element and iterates over the
    /// remaining, `prefix_range` accepts bounds for the lowest and highest elements of the
    /// `Prefix` itself, and iterates over those (inclusively or exclusively, depending on
    /// `PrefixBound`).
    /// There are some issues that distinguish these two, and blindly casting to `Vec<u8>` doesn't
    /// solve them.
    pub fn prefix_range<'c>(
        &self,
        store: &'c dyn Storage,
        min: Option<PrefixBound<'a, K::Prefix>>,
        max: Option<PrefixBound<'a, K::Prefix>>,
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = StdResult<(K::Output, T)>> + 'c>
    where
        T: 'c,
        'a: 'c,
        K: 'c,
        K::Output: 'static,
    {
        let mapped = namespaced_prefix_range(store, self.pk_namespace.as_slice(), min, max, order)
            .map(deserialize_kv::<K, T>);
        Box::new(mapped)
    }

    pub fn range_raw<'c>(
        &self,
        store: &'c dyn Storage,
        min: Option<Bound<'a, K>>,
        max: Option<Bound<'a, K>>,
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = StdResult<cosmwasm_std::Record<T>>> + 'c>
    where
        T: 'c,
    {
        self.no_prefix_raw().range_raw(store, min, max, order)
    }

    pub fn keys_raw<'c>(
        &self,
        store: &'c dyn Storage,
        min: Option<Bound<'a, K>>,
        max: Option<Bound<'a, K>>,
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = Vec<u8>> + 'c> {
        self.no_prefix_raw().keys_raw(store, min, max, order)
    }

    pub fn range<'c>(
        &self,
        store: &'c dyn Storage,
        min: Option<Bound<'a, K>>,
        max: Option<Bound<'a, K>>,
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = StdResult<(K::Output, T)>> + 'c>
    where
        T: 'c,
        K::Output: 'static,
    {
        self.no_prefix().range(store, min, max, order)
    }

    pub fn keys<'c>(
        &self,
        store: &'c dyn Storage,
        min: Option<Bound<'a, K>>,
        max: Option<Bound<'a, K>>,
        order: cosmwasm_std::Order,
    ) -> Box<dyn Iterator<Item = StdResult<K::Output>> + 'c>
    where
        T: 'c,
        K::Output: 'static,
    {
        self.no_prefix().keys(store, min, max, order)
    }

    fn no_prefix(&self) -> Prefix<K, T, K> {
        Prefix::new(self.pk_namespace.as_slice(), &[])
    }
}

#[cfg(test)]
mod test {

    use super::*;

    use crate::indexes::test::{index_string_tuple, index_tuple};
    use crate::{MultiIndex, UniqueIndex};
    use cosmwasm_std::testing::MockStorage;
    use cosmwasm_std::Order;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
    struct Data {
        pub name: String,
        pub last_name: String,
        pub age: u32,
    }

    struct DataIndexes<'a> {
        // Last type parameters are for signaling pk deserialization
        pub name: MultiIndex<'a, String, Data, String>,
        pub age: UniqueIndex<'a, u32, Data, String>,
        pub name_lastname: UniqueIndex<'a, (Vec<u8>, Vec<u8>), Data, String>,
    }

    // Future Note: this can likely be macro-derived
    impl IndexList<Data> for DataIndexes<'_> {
        fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Data>> + '_> {
            let v: Vec<&dyn Index<Data>> = vec![&self.name, &self.age, &self.name_lastname];
            Box::new(v.into_iter())
        }
    }

    // For composite multi index tests
    struct DataCompositeMultiIndex<'a> {
        // Last type parameter is for signaling pk deserialization
        pub name_age: MultiIndex<'a, (Vec<u8>, u32), Data, String>,
    }

    // Future Note: this can likely be macro-derived
    impl IndexList<Data> for DataCompositeMultiIndex<'_> {
        fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Data>> + '_> {
            let v: Vec<&dyn Index<Data>> = vec![&self.name_age];
            Box::new(v.into_iter())
        }
    }

    const DATA: IndexedMap<&str, Data, DataIndexes> = IndexedMap::new(
        "data",
        DataIndexes {
            name: MultiIndex::new(|_pk, d| d.name.clone(), "data", "data__name"),
            age: UniqueIndex::new(|d| d.age, "data__age"),
            name_lastname: UniqueIndex::new(
                |d| index_string_tuple(&d.name, &d.last_name),
                "data__name_lastname",
            ),
        },
    );

    fn save_data<'a>(store: &mut MockStorage) -> (Vec<&'a str>, Vec<Data>) {
        let mut pks = vec![];
        let mut datas = vec![];
        let data = Data {
            name: "Maria".to_string(),
            last_name: "Doe".to_string(),
            age: 42,
        };
        let pk = "1";
        DATA.save(store, pk, &data).unwrap();
        pks.push(pk);
        datas.push(data);

        // same name (multi-index), different last name, different age => ok
        let data = Data {
            name: "Maria".to_string(),
            last_name: "Williams".to_string(),
            age: 23,
        };
        let pk = "2";
        DATA.save(store, pk, &data).unwrap();
        pks.push(pk);
        datas.push(data);

        // different name, different last name, different age => ok
        let data = Data {
            name: "John".to_string(),
            last_name: "Wayne".to_string(),
            age: 32,
        };
        let pk = "3";
        DATA.save(store, pk, &data).unwrap();
        pks.push(pk);
        datas.push(data);

        let data = Data {
            name: "Maria Luisa".to_string(),
            last_name: "Rodriguez".to_string(),
            age: 12,
        };
        let pk = "4";
        DATA.save(store, pk, &data).unwrap();
        pks.push(pk);
        datas.push(data);

        let data = Data {
            name: "Marta".to_string(),
            last_name: "After".to_string(),
            age: 90,
        };
        let pk = "5";
        DATA.save(store, pk, &data).unwrap();
        pks.push(pk);
        datas.push(data);

        (pks, datas)
    }

    #[test]
    fn store_and_load_by_index() {
        let mut store = MockStorage::new();

        // save data
        let (pks, datas) = save_data(&mut store);
        let pk = pks[0];
        let data = &datas[0];

        // load it properly
        let loaded = DATA.load(&store, pk).unwrap();
        assert_eq!(*data, loaded);

        let count = DATA
            .idx
            .name
            .prefix("Maria".to_string())
            .range_raw(&store, None, None, Order::Ascending)
            .count();
        assert_eq!(2, count);

        // load it by secondary index
        let marias: Vec<_> = DATA
            .idx
            .name
            .prefix("Maria".to_string())
            .range_raw(&store, None, None, Order::Ascending)
            .collect::<StdResult<_>>()
            .unwrap();
        assert_eq!(2, marias.len());
        let (k, v) = &marias[0];
        assert_eq!(pk, String::from_slice(k).unwrap());
        assert_eq!(data, v);

        // other index doesn't match (1 byte after)
        let count = DATA
            .idx
            .name
            .prefix("Marib".to_string())
            .range_raw(&store, None, None, Order::Ascending)
            .count();
        assert_eq!(0, count);

        // other index doesn't match (1 byte before)
        let count = DATA
            .idx
            .name
            .prefix("Mari`".to_string())
            .range_raw(&store, None, None, Order::Ascending)
            .count();
        assert_eq!(0, count);

        // other index doesn't match (longer)
        let count = DATA
            .idx
            .name
            .prefix("Maria5".to_string())
            .range_raw(&store, None, None, Order::Ascending)
            .count();
        assert_eq!(0, count);

        // In a MultiIndex, the index key is composed by the index and the primary key.
        // Primary key may be empty (so that to iterate over all elements that match just the index)
        let key = ("Maria".to_string(), "".to_string());
        // Iterate using an inclusive bound over the key
        let marias = DATA
            .idx
            .name
            .range_raw(&store, Some(Bound::inclusive(key)), None, Order::Ascending)
            .collect::<StdResult<Vec<_>>>()
            .unwrap();
        // gets from the first "Maria" until the end
        assert_eq!(4, marias.len());

        // This is equivalent to using prefix_range
        let key = "Maria".to_string();
        let marias2 = DATA
            .idx
            .name
            .prefix_range_raw(
                &store,
                Some(PrefixBound::inclusive(key)),
                None,
                Order::Ascending,
            )
            .collect::<StdResult<Vec<_>>>()
            .unwrap();
        assert_eq!(4, marias2.len());
        assert_eq!(marias, marias2);

        // Build key including a non-empty pk
        let key = ("Maria".to_string(), "1".to_string());
        // Iterate using a (exclusive) bound over the key.
        // (Useful for pagination / continuation contexts).
        let count = DATA
            .idx
            .name
            .range_raw(&store, Some(Bound::exclusive(key)), None, Order::Ascending)
            .count();
        // gets from the 2nd "Maria" until the end
        assert_eq!(3, count);

        // index_key() over UniqueIndex works.
        let age_key = 23u32;
        // Iterate using a (inclusive) bound over the key.
        let count = DATA
            .idx
            .age
            .range_raw(
                &store,
                Some(Bound::inclusive(age_key)),
                None,
                Order::Ascending,
            )
            .count();
        // gets all the greater than or equal to 23 years old people
        assert_eq!(4, count);

        // match on proper age
        let proper = 42u32;
        let aged = DATA.idx.age.item(&store, proper).unwrap().unwrap();
        assert_eq!(pk, String::from_vec(aged.0).unwrap());
        assert_eq!(*data, aged.1);

        // no match on wrong age
        let too_old = 43u32;
        let aged = DATA.idx.age.item(&store, too_old).unwrap();
        assert_eq!(None, aged);
    }

    #[test]
    fn existence() {
        let mut store = MockStorage::new();
        let (pks, _) = save_data(&mut store);

        assert!(DATA.has(&store, pks[0]));
        assert!(!DATA.has(&store, "6"));
    }

    #[test]
    fn range_raw_simple_key_by_multi_index() {
        let mut store = MockStorage::new();

        // save data
        let data1 = Data {
            name: "Maria".to_string(),
            last_name: "".to_string(),
            age: 42,
        };
        let pk = "5627";
        DATA.save(&mut store, pk, &data1).unwrap();

        let data2 = Data {
            name: "Juan".to_string(),
            last_name: "Perez".to_string(),
            age: 13,
        };
        let pk = "5628";
        DATA.save(&mut store, pk, &data2).unwrap();

        let data3 = Data {
            name: "Maria".to_string(),
            last_name: "Williams".to_string(),
            age: 24,
        };
        let pk = "5629";
        DATA.save(&mut store, pk, &data3).unwrap();

        let data4 = Data {
            name: "Maria Luisa".to_string(),
            last_name: "Bemberg".to_string(),
            age: 12,
        };
        let pk = "5630";
        DATA.save(&mut store, pk, &data4).unwrap();

        let marias: Vec<_> = DATA
            .idx
            .name
            .prefix("Maria".to_string())
            .range_raw(&store, None, None, Order::Descending)
            .collect::<StdResult<_>>()
            .unwrap();
        let count = marias.len();
        assert_eq!(2, count);

        // Pks, sorted by (descending) pk
        assert_eq!(marias[0].0, b"5629");
        assert_eq!(marias[1].0, b"5627");
        // Data is correct
        assert_eq!(marias[0].1, data3);
        assert_eq!(marias[1].1, data1);
    }

    #[test]
    fn range_simple_key_by_multi_index() {
        let mut store = MockStorage::new();

        // save data
        let data1 = Data {
            name: "Maria".to_string(),
            last_name: "".to_string(),
            age: 42,
        };
        let pk = "5627";
        DATA.save(&mut store, pk, &data1).unwrap();

        let data2 = Data {
            name: "Juan".to_string(),
            last_name: "Perez".to_string(),
            age: 13,
        };
        let pk = "5628";
        DATA.save(&mut store, pk, &data2).unwrap();

        let data3 = Data {
            name: "Maria".to_string(),
            last_name: "Williams".to_string(),
            age: 24,
        };
        let pk = "5629";
        DATA.save(&mut store, pk, &data3).unwrap();

        let data4 = Data {
            name: "Maria Luisa".to_string(),
            last_name: "Bemberg".to_string(),
            age: 12,
        };
        let pk = "5630";
        DATA.save(&mut store, pk, &data4).unwrap();

        let marias: Vec<_> = DATA
            .idx
            .name
            .prefix("Maria".to_string())
            .range(&store, None, None, Order::Descending)
            .collect::<StdResult<_>>()
            .unwrap();
        let count = marias.len();
        assert_eq!(2, count);

        // Pks, sorted by (descending) pk
        assert_eq!(marias[0].0, "5629");
        assert_eq!(marias[1].0, "5627");
        // Data is correct
        assert_eq!(marias[0].1, data3);
        assert_eq!(marias[1].1, data1);
    }

    #[test]
    fn range_raw_composite_key_by_multi_index() {
        let mut store = MockStorage::new();

        let indexes = DataCompositeMultiIndex {
            name_age: MultiIndex::new(
                |_pk, d| index_tuple(&d.name, d.age),
                "data",
                "data__name_age",
            ),
        };
        let map = IndexedMap::new("data", indexes);

        // save data
        let data1 = Data {
            name: "Maria".to_string(),
            last_name: "".to_string(),
            age: 42,
        };
        let pk1: &[u8] = b"5627";
        map.save(&mut store, pk1, &data1).unwrap();

        let data2 = Data {
            name: "Juan".to_string(),
            last_name: "Perez".to_string(),
            age: 13,
        };
        let pk2: &[u8] = b"5628";
        map.save(&mut store, pk2, &data2).unwrap();

        let data3 = Data {
            name: "Maria".to_string(),
            last_name: "Young".to_string(),
            age: 24,
        };
        let pk3: &[u8] = b"5629";
        map.save(&mut store, pk3, &data3).unwrap();

        let data4 = Data {
            name: "Maria Luisa".to_string(),
            last_name: "Bemberg".to_string(),
            age: 43,
        };
        let pk4: &[u8] = b"5630";
        map.save(&mut store, pk4, &data4).unwrap();

        let marias: Vec<_> = map
            .idx
            .name_age
            .sub_prefix(b"Maria".to_vec())
            .range_raw(&store, None, None, Order::Descending)
            .collect::<StdResult<_>>()
            .unwrap();
        let count = marias.len();
        assert_eq!(2, count);

        // Pks, sorted by (descending) age
        assert_eq!(pk1, marias[0].0);
        assert_eq!(pk3, marias[1].0);

        // Data
        assert_eq!(data1, marias[0].1);
        assert_eq!(data3, marias[1].1);
    }

    #[test]
    fn range_composite_key_by_multi_index() {
        let mut store = MockStorage::new();

        let indexes = DataCompositeMultiIndex {
            name_age: MultiIndex::new(
                |_pk, d| index_tuple(&d.name, d.age),
                "data",
                "data__name_age",
            ),
        };
        let map = IndexedMap::new("data", indexes);

        // save data
        let data1 = Data {
            name: "Maria".to_string(),
            last_name: "".to_string(),
            age: 42,
        };
        let pk1 = "5627";
        map.save(&mut store, pk1, &data1).unwrap();

        let data2 = Data {
            name: "Juan".to_string(),
            last_name: "Perez".to_string(),
            age: 13,
        };
        let pk2 = "5628";
        map.save(&mut store, pk2, &data2).unwrap();

        let data3 = Data {
            name: "Maria".to_string(),
            last_name: "Young".to_string(),
            age: 24,
        };
        let pk3 = "5629";
        map.save(&mut store, pk3, &data3).unwrap();

        let data4 = Data {
            name: "Maria Luisa".to_string(),
            last_name: "Bemberg".to_string(),
            age: 43,
        };
        let pk4 = "5630";
        map.save(&mut store, pk4, &data4).unwrap();

        let marias: Vec<_> = map
            .idx
            .name_age
            .sub_prefix(b"Maria".to_vec())
            .range(&store, None, None, Order::Descending)
            .collect::<StdResult<_>>()
            .unwrap();
        let count = marias.len();
        assert_eq!(2, count);

        // Pks, sorted by (descending) age
        assert_eq!(pk1, marias[0].0);
        assert_eq!(pk3, marias[1].0);

        // Data
        assert_eq!(data1, marias[0].1);
        assert_eq!(data3, marias[1].1);
    }

    #[test]
    fn unique_index_enforced() {
        let mut store = MockStorage::new();

        // save data
        let (pks, datas) = save_data(&mut store);

        // different name, different last name, same age => error
        let data5 = Data {
            name: "Marcel".to_string(),
            last_name: "Laurens".to_string(),
            age: 42,
        };
        let pk5 = "4";

        // enforce this returns some error
        DATA.save(&mut store, pk5, &data5).unwrap_err();

        // query by unique key
        // match on proper age
        let age42 = 42u32;
        let (k, v) = DATA.idx.age.item(&store, age42).unwrap().unwrap();
        assert_eq!(String::from_vec(k).unwrap(), pks[0]);
        assert_eq!(v.name, datas[0].name);
        assert_eq!(v.age, datas[0].age);

        // match on other age
        let age23 = 23u32;
        let (k, v) = DATA.idx.age.item(&store, age23).unwrap().unwrap();
        assert_eq!(String::from_vec(k).unwrap(), pks[1]);
        assert_eq!(v.name, datas[1].name);
        assert_eq!(v.age, datas[1].age);

        // if we delete the first one, we can add the blocked one
        DATA.remove(&mut store, pks[0]).unwrap();
        DATA.save(&mut store, pk5, &data5).unwrap();
        // now 42 is the new owner
        let (k, v) = DATA.idx.age.item(&store, age42).unwrap().unwrap();
        assert_eq!(String::from_vec(k).unwrap(), pk5);
        assert_eq!(v.name, data5.name);
        assert_eq!(v.age, data5.age);
    }

    #[test]
    fn unique_index_enforced_composite_key() {
        let mut store = MockStorage::new();

        // save data
        save_data(&mut store);

        // same name, same lastname => error
        let data5 = Data {
            name: "Maria".to_string(),
            last_name: "Doe".to_string(),
            age: 24,
        };
        let pk5 = "5";
        // enforce this returns some error
        DATA.save(&mut store, pk5, &data5).unwrap_err();
    }

    #[test]
    fn remove_and_update_reflected_on_indexes() {
        let mut store = MockStorage::new();

        let name_count = |store: &MockStorage, name: &str| -> usize {
            DATA.idx
                .name
                .prefix(name.to_string())
                .keys_raw(store, None, None, Order::Ascending)
                .count()
        };

        // save data
        let (pks, _) = save_data(&mut store);

        // find 2 Marias, 1 John, and no Mary
        assert_eq!(name_count(&store, "Maria"), 2);
        assert_eq!(name_count(&store, "John"), 1);
        assert_eq!(name_count(&store, "Maria Luisa"), 1);
        assert_eq!(name_count(&store, "Mary"), 0);

        // remove maria 2
        DATA.remove(&mut store, pks[1]).unwrap();

        // change john to mary
        DATA.update(&mut store, pks[2], |d| -> StdResult<_> {
            let mut x = d.unwrap();
            assert_eq!(&x.name, "John");
            x.name = "Mary".to_string();
            Ok(x)
        })
        .unwrap();

        // find 1 maria, 1 maria luisa, no john, and 1 mary
        assert_eq!(name_count(&store, "Maria"), 1);
        assert_eq!(name_count(&store, "Maria Luisa"), 1);
        assert_eq!(name_count(&store, "John"), 0);
        assert_eq!(name_count(&store, "Mary"), 1);
    }

    #[test]
    fn range_raw_simple_key_by_unique_index() {
        let mut store = MockStorage::new();

        // save data
        let (pks, datas) = save_data(&mut store);

        let res: StdResult<Vec<_>> = DATA
            .idx
            .age
            .range_raw(&store, None, None, Order::Ascending)
            .collect();
        let ages = res.unwrap();

        let count = ages.len();
        assert_eq!(5, count);

        // The pks, sorted by age ascending
        assert_eq!(pks[3], String::from_slice(&ages[0].0).unwrap()); // 12
        assert_eq!(pks[1], String::from_slice(&ages[1].0).unwrap()); // 23
        assert_eq!(pks[2], String::from_slice(&ages[2].0).unwrap()); // 32
        assert_eq!(pks[0], String::from_slice(&ages[3].0).unwrap()); // 42
        assert_eq!(pks[4], String::from_slice(&ages[4].0).unwrap()); // 90

        // The associated data
        assert_eq!(datas[3], ages[0].1);
        assert_eq!(datas[1], ages[1].1);
        assert_eq!(datas[2], ages[2].1);
        assert_eq!(datas[0], ages[3].1);
        assert_eq!(datas[4], ages[4].1);
    }

    #[test]
    fn range_simple_key_by_unique_index() {
        let mut store = MockStorage::new();

        // save data
        let (pks, datas) = save_data(&mut store);

        let res: StdResult<Vec<_>> = DATA
            .idx
            .age
            .range(&store, None, None, Order::Ascending)
            .collect();
        let ages = res.unwrap();

        let count = ages.len();
        assert_eq!(5, count);

        // The pks, sorted by age ascending
        assert_eq!(pks[3], ages[0].0);
        assert_eq!(pks[1], ages[1].0);
        assert_eq!(pks[2], ages[2].0);
        assert_eq!(pks[0], ages[3].0);
        assert_eq!(pks[4], ages[4].0);

        // The associated data
        assert_eq!(datas[3], ages[0].1);
        assert_eq!(datas[1], ages[1].1);
        assert_eq!(datas[2], ages[2].1);
        assert_eq!(datas[0], ages[3].1);
        assert_eq!(datas[4], ages[4].1);
    }

    #[test]
    fn range_raw_composite_key_by_unique_index() {
        let mut store = MockStorage::new();

        // save data
        let (pks, datas) = save_data(&mut store);

        let res: StdResult<Vec<_>> = DATA
            .idx
            .name_lastname
            .prefix(b"Maria".to_vec())
            .range_raw(&store, None, None, Order::Ascending)
            .collect();
        let marias = res.unwrap();

        // Only two people are called "Maria"
        let count = marias.len();
        assert_eq!(2, count);

        // The pks
        assert_eq!(pks[0], String::from_slice(&marias[0].0).unwrap());
        assert_eq!(pks[1], String::from_slice(&marias[1].0).unwrap());

        // The associated data
        assert_eq!(datas[0], marias[0].1);
        assert_eq!(datas[1], marias[1].1);
    }

    #[test]
    fn range_composite_key_by_unique_index() {
        let mut store = MockStorage::new();

        // save data
        let (pks, datas) = save_data(&mut store);

        let res: StdResult<Vec<_>> = DATA
            .idx
            .name_lastname
            .prefix(b"Maria".to_vec())
            .range(&store, None, None, Order::Ascending)
            .collect();
        let marias = res.unwrap();

        // Only two people are called "Maria"
        let count = marias.len();
        assert_eq!(2, count);

        // The pks
        assert_eq!(pks[0], marias[0].0);
        assert_eq!(pks[1], marias[1].0);

        // The associated data
        assert_eq!(datas[0], marias[0].1);
        assert_eq!(datas[1], marias[1].1);
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn range_simple_string_key() {
        let mut store = MockStorage::new();

        // save data
        let (pks, datas) = save_data(&mut store);

        // let's try to iterate!
        let all: StdResult<Vec<_>> = DATA.range(&store, None, None, Order::Ascending).collect();
        let all = all.unwrap();
        assert_eq!(
            all,
            pks.clone()
                .into_iter()
                .map(str::to_string)
                .zip(datas.clone().into_iter())
                .collect::<Vec<_>>()
        );

        // let's try to iterate over a range
        let all: StdResult<Vec<_>> = DATA
            .range(&store, Some(Bound::inclusive("3")), None, Order::Ascending)
            .collect();
        let all = all.unwrap();
        assert_eq!(
            all,
            pks.into_iter()
                .map(str::to_string)
                .zip(datas.into_iter())
                .rev()
                .take(3)
                .rev()
                .collect::<Vec<_>>()
        );
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn prefix_simple_string_key() {
        let mut store = MockStorage::new();

        // save data
        let (pks, datas) = save_data(&mut store);

        // Let's prefix and iterate.
        // This is similar to calling range() directly, but added here for completeness / prefix
        // type checks
        let all: StdResult<Vec<_>> = DATA
            .prefix(())
            .range(&store, None, None, Order::Ascending)
            .collect();
        let all = all.unwrap();
        assert_eq!(
            all,
            pks.clone()
                .into_iter()
                .map(str::to_string)
                .zip(datas.into_iter())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn prefix_composite_key() {
        let mut store = MockStorage::new();

        let indexes = DataCompositeMultiIndex {
            name_age: MultiIndex::new(
                |_pk, d| index_tuple(&d.name, d.age),
                "data",
                "data__name_age",
            ),
        };
        let map = IndexedMap::new("data", indexes);

        // save data
        let data1 = Data {
            name: "Maria".to_string(),
            last_name: "".to_string(),
            age: 42,
        };
        let pk1 = ("1", "5627");
        map.save(&mut store, pk1, &data1).unwrap();

        let data2 = Data {
            name: "Juan".to_string(),
            last_name: "Perez".to_string(),
            age: 13,
        };
        let pk2 = ("2", "5628");
        map.save(&mut store, pk2, &data2).unwrap();

        let data3 = Data {
            name: "Maria".to_string(),
            last_name: "Young".to_string(),
            age: 24,
        };
        let pk3 = ("2", "5629");
        map.save(&mut store, pk3, &data3).unwrap();

        let data4 = Data {
            name: "Maria Luisa".to_string(),
            last_name: "Bemberg".to_string(),
            age: 43,
        };
        let pk4 = ("3", "5630");
        map.save(&mut store, pk4, &data4).unwrap();

        // let's prefix and iterate
        let result: StdResult<Vec<_>> = map
            .prefix("2")
            .range(&store, None, None, Order::Ascending)
            .collect();
        let result = result.unwrap();
        assert_eq!(
            result,
            [("5628".to_string(), data2), ("5629".to_string(), data3),]
        );
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn prefix_triple_key() {
        let mut store = MockStorage::new();

        let indexes = DataCompositeMultiIndex {
            name_age: MultiIndex::new(
                |_pk, d| index_tuple(&d.name, d.age),
                "data",
                "data__name_age",
            ),
        };
        let map = IndexedMap::new("data", indexes);

        // save data
        let data1 = Data {
            name: "Maria".to_string(),
            last_name: "".to_string(),
            age: 42,
        };
        let pk1 = ("1", "1", "5627");
        map.save(&mut store, pk1, &data1).unwrap();

        let data2 = Data {
            name: "Juan".to_string(),
            last_name: "Perez".to_string(),
            age: 13,
        };
        let pk2 = ("1", "2", "5628");
        map.save(&mut store, pk2, &data2).unwrap();

        let data3 = Data {
            name: "Maria".to_string(),
            last_name: "Young".to_string(),
            age: 24,
        };
        let pk3 = ("2", "1", "5629");
        map.save(&mut store, pk3, &data3).unwrap();

        let data4 = Data {
            name: "Maria Luisa".to_string(),
            last_name: "Bemberg".to_string(),
            age: 43,
        };
        let pk4 = ("2", "2", "5630");
        map.save(&mut store, pk4, &data4).unwrap();

        // let's prefix and iterate
        let result: StdResult<Vec<_>> = map
            .prefix(("1", "2"))
            .range(&store, None, None, Order::Ascending)
            .collect();
        let result = result.unwrap();
        assert_eq!(result, [("5628".to_string(), data2),]);
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn sub_prefix_triple_key() {
        let mut store = MockStorage::new();

        let indexes = DataCompositeMultiIndex {
            name_age: MultiIndex::new(
                |_pk, d| index_tuple(&d.name, d.age),
                "data",
                "data__name_age",
            ),
        };
        let map = IndexedMap::new("data", indexes);

        // save data
        let data1 = Data {
            name: "Maria".to_string(),
            last_name: "".to_string(),
            age: 42,
        };
        let pk1 = ("1", "1", "5627");
        map.save(&mut store, pk1, &data1).unwrap();

        let data2 = Data {
            name: "Juan".to_string(),
            last_name: "Perez".to_string(),
            age: 13,
        };
        let pk2 = ("1", "2", "5628");
        map.save(&mut store, pk2, &data2).unwrap();

        let data3 = Data {
            name: "Maria".to_string(),
            last_name: "Young".to_string(),
            age: 24,
        };
        let pk3 = ("2", "1", "5629");
        map.save(&mut store, pk3, &data3).unwrap();

        let data4 = Data {
            name: "Maria Luisa".to_string(),
            last_name: "Bemberg".to_string(),
            age: 43,
        };
        let pk4 = ("2", "2", "5630");
        map.save(&mut store, pk4, &data4).unwrap();

        // let's sub-prefix and iterate
        let result: StdResult<Vec<_>> = map
            .sub_prefix("1")
            .range(&store, None, None, Order::Ascending)
            .collect();
        let result = result.unwrap();
        assert_eq!(
            result,
            [
                (("1".to_string(), "5627".to_string()), data1),
                (("2".to_string(), "5628".to_string()), data2),
            ]
        );
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn prefix_range_simple_key() {
        let mut store = MockStorage::new();

        let indexes = DataCompositeMultiIndex {
            name_age: MultiIndex::new(
                |_pk, d| index_tuple(&d.name, d.age),
                "data",
                "data__name_age",
            ),
        };
        let map = IndexedMap::new("data", indexes);

        // save data
        let data1 = Data {
            name: "Maria".to_string(),
            last_name: "".to_string(),
            age: 42,
        };
        let pk1 = ("1", "5627");
        map.save(&mut store, pk1, &data1).unwrap();

        let data2 = Data {
            name: "Juan".to_string(),
            last_name: "Perez".to_string(),
            age: 13,
        };
        let pk2 = ("2", "5628");
        map.save(&mut store, pk2, &data2).unwrap();

        let data3 = Data {
            name: "Maria".to_string(),
            last_name: "Young".to_string(),
            age: 24,
        };
        let pk3 = ("2", "5629");
        map.save(&mut store, pk3, &data3).unwrap();

        let data4 = Data {
            name: "Maria Luisa".to_string(),
            last_name: "Bemberg".to_string(),
            age: 43,
        };
        let pk4 = ("3", "5630");
        map.save(&mut store, pk4, &data4).unwrap();

        // let's prefix-range and iterate
        let result: StdResult<Vec<_>> = map
            .prefix_range(
                &store,
                Some(PrefixBound::inclusive("2")),
                None,
                Order::Ascending,
            )
            .collect();
        let result = result.unwrap();
        assert_eq!(
            result,
            [
                (("2".to_string(), "5628".to_string()), data2.clone()),
                (("2".to_string(), "5629".to_string()), data3.clone()),
                (("3".to_string(), "5630".to_string()), data4)
            ]
        );

        // let's try to iterate over a more restrictive prefix-range!
        let result: StdResult<Vec<_>> = map
            .prefix_range(
                &store,
                Some(PrefixBound::inclusive("2")),
                Some(PrefixBound::exclusive("3")),
                Order::Ascending,
            )
            .collect();
        let result = result.unwrap();
        assert_eq!(
            result,
            [
                (("2".to_string(), "5628".to_string()), data2),
                (("2".to_string(), "5629".to_string()), data3),
            ]
        );
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn prefix_range_triple_key() {
        let mut store = MockStorage::new();

        let indexes = DataCompositeMultiIndex {
            name_age: MultiIndex::new(
                |_pk, d| index_tuple(&d.name, d.age),
                "data",
                "data__name_age",
            ),
        };
        let map = IndexedMap::new("data", indexes);

        // save data
        let data1 = Data {
            name: "Maria".to_string(),
            last_name: "".to_string(),
            age: 42,
        };
        let pk1 = ("1", "1", "5627");
        map.save(&mut store, pk1, &data1).unwrap();

        let data2 = Data {
            name: "Juan".to_string(),
            last_name: "Perez".to_string(),
            age: 13,
        };
        let pk2 = ("1", "2", "5628");
        map.save(&mut store, pk2, &data2).unwrap();

        let data3 = Data {
            name: "Maria".to_string(),
            last_name: "Young".to_string(),
            age: 24,
        };
        let pk3 = ("2", "1", "5629");
        map.save(&mut store, pk3, &data3).unwrap();

        let data4 = Data {
            name: "Maria Luisa".to_string(),
            last_name: "Bemberg".to_string(),
            age: 43,
        };
        let pk4 = ("2", "2", "5630");
        map.save(&mut store, pk4, &data4).unwrap();

        // let's prefix-range and iterate
        let result: StdResult<Vec<_>> = map
            .prefix_range(
                &store,
                Some(PrefixBound::inclusive(("1", "2"))),
                None,
                Order::Ascending,
            )
            .collect();
        let result = result.unwrap();
        assert_eq!(
            result,
            [
                (
                    ("1".to_string(), "2".to_string(), "5628".to_string()),
                    data2.clone()
                ),
                (
                    ("2".to_string(), "1".to_string(), "5629".to_string()),
                    data3.clone()
                ),
                (
                    ("2".to_string(), "2".to_string(), "5630".to_string()),
                    data4
                )
            ]
        );

        // let's prefix-range over inclusive bounds on both sides
        let result: StdResult<Vec<_>> = map
            .prefix_range(
                &store,
                Some(PrefixBound::inclusive(("1", "2"))),
                Some(PrefixBound::inclusive(("2", "1"))),
                Order::Ascending,
            )
            .collect();
        let result = result.unwrap();
        assert_eq!(
            result,
            [
                (
                    ("1".to_string(), "2".to_string(), "5628".to_string()),
                    data2
                ),
                (
                    ("2".to_string(), "1".to_string(), "5629".to_string()),
                    data3
                ),
            ]
        );
    }

    mod bounds_unique_index {
        use super::*;

        struct Indexes<'a> {
            secondary: UniqueIndex<'a, u64, u64, ()>,
        }

        impl IndexList<u64> for Indexes<'_> {
            fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<u64>> + '_> {
                let v: Vec<&dyn Index<u64>> = vec![&self.secondary];
                Box::new(v.into_iter())
            }
        }

        #[test]
        #[cfg(feature = "iterator")]
        fn composite_key_query() {
            let indexes = Indexes {
                secondary: UniqueIndex::new(|secondary| *secondary, "test_map__secondary"),
            };
            let map = IndexedMap::<&str, u64, Indexes>::new("test_map", indexes);
            let mut store = MockStorage::new();

            map.save(&mut store, "one", &1).unwrap();
            map.save(&mut store, "two", &2).unwrap();
            map.save(&mut store, "three", &3).unwrap();

            // Inclusive bound
            let items: Vec<_> = map
                .idx
                .secondary
                .range_raw(&store, None, Some(Bound::inclusive(1u64)), Order::Ascending)
                .collect::<Result<_, _>>()
                .unwrap();

            // Strip the index from values (for simpler comparison)
            let items: Vec<_> = items.into_iter().map(|(_, v)| v).collect();

            assert_eq!(items, vec![1]);

            // Exclusive bound
            let items: Vec<_> = map
                .idx
                .secondary
                .range(&store, Some(Bound::exclusive(2u64)), None, Order::Ascending)
                .collect::<Result<_, _>>()
                .unwrap();

            assert_eq!(items, vec![((), 3)]);
        }
    }

    mod bounds_multi_index {
        use super::*;

        struct Indexes<'a> {
            // The last type param must match the `IndexedMap` primary key type, below
            secondary: MultiIndex<'a, u64, u64, &'a str>,
        }

        impl IndexList<u64> for Indexes<'_> {
            fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<u64>> + '_> {
                let v: Vec<&dyn Index<u64>> = vec![&self.secondary];
                Box::new(v.into_iter())
            }
        }

        #[test]
        #[cfg(feature = "iterator")]
        fn composite_key_query() {
            let indexes = Indexes {
                secondary: MultiIndex::new(
                    |_pk, secondary| *secondary,
                    "test_map",
                    "test_map__secondary",
                ),
            };
            let map = IndexedMap::<&str, u64, Indexes>::new("test_map", indexes);
            let mut store = MockStorage::new();

            map.save(&mut store, "one", &1).unwrap();
            map.save(&mut store, "two", &2).unwrap();
            map.save(&mut store, "two2", &2).unwrap();
            map.save(&mut store, "three", &3).unwrap();

            // Inclusive prefix-bound
            let items: Vec<_> = map
                .idx
                .secondary
                .prefix_range_raw(
                    &store,
                    None,
                    Some(PrefixBound::inclusive(1u64)),
                    Order::Ascending,
                )
                .collect::<Result<_, _>>()
                .unwrap();

            // Strip the index from values (for simpler comparison)
            let items: Vec<_> = items.into_iter().map(|(_, v)| v).collect();

            assert_eq!(items, vec![1]);

            // Exclusive bound (used for pagination)
            // Range over the index specifying a primary key (multi-index key includes the pk)
            let items: Vec<_> = map
                .idx
                .secondary
                .range(
                    &store,
                    Some(Bound::exclusive((2u64, "two"))),
                    None,
                    Order::Ascending,
                )
                .collect::<Result<_, _>>()
                .unwrap();

            assert_eq!(
                items,
                vec![("two2".to_string(), 2), ("three".to_string(), 3)]
            );
        }
    }

    mod pk_multi_index {
        use super::*;
        use cosmwasm_std::{Addr, Uint128};

        struct Indexes<'a> {
            // The last type param must match the `IndexedMap` primary key type below
            spender: MultiIndex<'a, Addr, Uint128, (Addr, Addr)>,
        }

        impl IndexList<Uint128> for Indexes<'_> {
            fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Uint128>> + '_> {
                let v: Vec<&dyn Index<Uint128>> = vec![&self.spender];
                Box::new(v.into_iter())
            }
        }

        #[test]
        #[cfg(feature = "iterator")]
        fn pk_based_index() {
            fn pk_index(pk: &[u8]) -> Addr {
                let (_owner, spender) = <(Addr, Addr)>::from_slice(pk).unwrap(); // mustn't fail
                spender
            }

            let indexes = Indexes {
                spender: MultiIndex::new(
                    |pk, _allow| pk_index(pk),
                    "allowances",
                    "allowances__spender",
                ),
            };
            let map = IndexedMap::<(&Addr, &Addr), Uint128, Indexes>::new("allowances", indexes);
            let mut store = MockStorage::new();

            map.save(
                &mut store,
                (&Addr::unchecked("owner1"), &Addr::unchecked("spender1")),
                &Uint128::new(11),
            )
            .unwrap();
            map.save(
                &mut store,
                (&Addr::unchecked("owner1"), &Addr::unchecked("spender2")),
                &Uint128::new(12),
            )
            .unwrap();
            map.save(
                &mut store,
                (&Addr::unchecked("owner2"), &Addr::unchecked("spender1")),
                &Uint128::new(21),
            )
            .unwrap();

            // Iterate over the main values
            let items: Vec<_> = map
                .range_raw(&store, None, None, Order::Ascending)
                .collect::<Result<_, _>>()
                .unwrap();

            // Strip the index from values (for simpler comparison)
            let items: Vec<_> = items.into_iter().map(|(_, v)| v.u128()).collect();

            assert_eq!(items, vec![11, 12, 21]);

            // Iterate over the indexed values
            let items: Vec<_> = map
                .idx
                .spender
                .range_raw(&store, None, None, Order::Ascending)
                .collect::<Result<_, _>>()
                .unwrap();

            // Strip the index from values (for simpler comparison)
            let items: Vec<_> = items.into_iter().map(|(_, v)| v.u128()).collect();

            assert_eq!(items, vec![11, 21, 12]);

            // Prefix over the main values
            let items: Vec<_> = map
                .prefix(&Addr::unchecked("owner1"))
                .range_raw(&store, None, None, Order::Ascending)
                .collect::<Result<_, _>>()
                .unwrap();

            // Strip the index from values (for simpler comparison)
            let items: Vec<_> = items.into_iter().map(|(_, v)| v.u128()).collect();

            assert_eq!(items, vec![11, 12]);

            // Prefix over the indexed values
            let items: Vec<_> = map
                .idx
                .spender
                .prefix(Addr::unchecked("spender1"))
                .range_raw(&store, None, None, Order::Ascending)
                .collect::<Result<_, _>>()
                .unwrap();

            // Strip the index from values (for simpler comparison)
            let items: Vec<_> = items.into_iter().map(|(_, v)| v.u128()).collect();

            assert_eq!(items, vec![11, 21]);

            // Prefix over the indexed values, and deserialize primary key as well
            let items: Vec<_> = map
                .idx
                .spender
                .prefix(Addr::unchecked("spender2"))
                .range(&store, None, None, Order::Ascending)
                .collect::<Result<_, _>>()
                .unwrap();

            assert_eq!(
                items,
                vec![(
                    (Addr::unchecked("owner1"), Addr::unchecked("spender2")),
                    Uint128::new(12)
                )]
            );
        }
    }

    #[test]
    fn clear_works() {
        let mut storage = MockStorage::new();
        let (pks, _) = save_data(&mut storage);

        DATA.clear(&mut storage);

        for key in pks {
            assert!(!DATA.has(&storage, key));
        }
    }

    #[test]
    fn is_empty_works() {
        let mut storage = MockStorage::new();

        assert!(DATA.is_empty(&storage));

        save_data(&mut storage);

        assert!(!DATA.is_empty(&storage));
    }
}
