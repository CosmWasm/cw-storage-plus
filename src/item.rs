use serde::de::DeserializeOwned;
use serde::Serialize;
use std::marker::PhantomData;

use cosmwasm_std::{
    from_json, to_json_vec, Addr, CustomQuery, QuerierWrapper, StdError, StdResult, Storage,
    WasmQuery,
};

use crate::{helpers::not_found_object_info, namespace::Namespace};

/// Item stores one typed item at the given key.
/// This is an analog of Singleton.
/// It functions the same way as Path does but doesn't use a Vec and thus has a const fn constructor.
pub struct Item<T> {
    // this is full key - no need to length-prefix it, we only store one item
    storage_key: Namespace,
    // see https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-type-parameters for why this is needed
    data_type: PhantomData<T>,
}

impl<T> Item<T> {
    /// Creates a new [`Item`] with the given storage key. This is a const fn only suitable
    /// when you have a static string slice.
    pub const fn new(storage_key: &'static str) -> Self {
        Item {
            storage_key: Namespace::from_static_str(storage_key),
            data_type: PhantomData,
        }
    }

    /// Creates a new [`Item`] with the given storage key. Use this if you might need to handle
    /// a dynamic string. Otherwise, you might prefer [`Item::new`].
    pub fn new_dyn(storage_key: impl Into<Namespace>) -> Self {
        Item {
            storage_key: storage_key.into(),
            data_type: PhantomData,
        }
    }
}

impl<T> Item<T>
where
    T: Serialize + DeserializeOwned,
{
    // this gets the path of the data to use elsewhere
    pub fn as_slice(&self) -> &[u8] {
        self.storage_key.as_slice()
    }

    /// save will serialize the model and store, returns an error on serialization issues
    pub fn save(&self, store: &mut dyn Storage, data: &T) -> StdResult<()> {
        store.set(self.storage_key.as_slice(), &to_json_vec(data)?);
        Ok(())
    }

    pub fn remove(&self, store: &mut dyn Storage) {
        store.remove(self.storage_key.as_slice());
    }

    /// load will return an error if no data is set at the given key, or on parse error
    pub fn load(&self, store: &dyn Storage) -> StdResult<T> {
        if let Some(value) = store.get(self.storage_key.as_slice()) {
            from_json(value)
        } else {
            let object_info = not_found_object_info::<T>(self.storage_key.as_slice());
            Err(StdError::not_found(object_info))
        }
    }

    /// may_load will parse the data stored at the key if present, returns `Ok(None)` if no data there.
    /// returns an error on issues parsing
    pub fn may_load(&self, store: &dyn Storage) -> StdResult<Option<T>> {
        let value = store.get(self.storage_key.as_slice());
        value.map(|v| from_json(v)).transpose()
    }

    /// Returns `true` if data is stored at the key, `false` otherwise.
    pub fn exists(&self, store: &dyn Storage) -> bool {
        store.get(self.storage_key.as_slice()).is_some()
    }

    /// Loads the data, perform the specified action, and store the result
    /// in the database. This is shorthand for some common sequences, which may be useful.
    ///
    /// It assumes, that data was initialized before, and if it doesn't exist, `Err(StdError::NotFound)`
    /// is returned.
    pub fn update<A, E>(&self, store: &mut dyn Storage, action: A) -> Result<T, E>
    where
        A: FnOnce(T) -> Result<T, E>,
        E: From<StdError>,
    {
        let input = self.load(store)?;
        let output = action(input)?;
        self.save(store, &output)?;
        Ok(output)
    }

    /// If you import the proper Item from the remote contract, this will let you read the data
    /// from a remote contract in a type-safe way using WasmQuery::RawQuery.
    ///
    /// Note that we expect an Item to be set, and error if there is no data there
    pub fn query<Q: CustomQuery>(
        &self,
        querier: &QuerierWrapper<Q>,
        remote_contract: Addr,
    ) -> StdResult<T> {
        let request = WasmQuery::Raw {
            contract_addr: remote_contract.into(),
            key: (self.storage_key.as_slice()).into(),
        };
        querier.query(&request.into())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::testing::MockStorage;
    use serde::{Deserialize, Serialize};

    use cosmwasm_std::{to_json_vec, OverflowError, OverflowOperation, StdError};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Config {
        pub owner: String,
        pub max_tokens: i32,
    }

    // note const constructor rather than 2 funcs with Singleton
    const CONFIG: Item<Config> = Item::new("config");

    #[test]
    fn save_and_load() {
        let mut store = MockStorage::new();

        assert!(CONFIG.load(&store).is_err());
        assert_eq!(CONFIG.may_load(&store).unwrap(), None);

        let cfg = Config {
            owner: "admin".to_string(),
            max_tokens: 1234,
        };
        CONFIG.save(&mut store, &cfg).unwrap();

        assert_eq!(cfg, CONFIG.load(&store).unwrap());
    }

    #[test]
    fn owned_key_works() {
        let mut store = MockStorage::new();

        for i in 0..3 {
            let key = format!("key{}", i);
            let item = Item::new_dyn(key);
            item.save(&mut store, &i).unwrap();
        }

        assert_eq!(store.get(b"key0").unwrap(), b"0");
        assert_eq!(store.get(b"key1").unwrap(), b"1");
        assert_eq!(store.get(b"key2").unwrap(), b"2");
    }

    #[test]
    fn exists_works() {
        let mut store = MockStorage::new();

        assert!(!CONFIG.exists(&store));

        let cfg = Config {
            owner: "admin".to_string(),
            max_tokens: 1234,
        };
        CONFIG.save(&mut store, &cfg).unwrap();

        assert!(CONFIG.exists(&store));

        const OPTIONAL: Item<Option<u32>> = Item::new("optional");

        assert!(!OPTIONAL.exists(&store));

        OPTIONAL.save(&mut store, &None).unwrap();

        assert!(OPTIONAL.exists(&store));
    }

    #[test]
    fn remove_works() {
        let mut store = MockStorage::new();

        // store data
        let cfg = Config {
            owner: "admin".to_string(),
            max_tokens: 1234,
        };
        CONFIG.save(&mut store, &cfg).unwrap();
        assert_eq!(cfg, CONFIG.load(&store).unwrap());

        // remove it and loads None
        CONFIG.remove(&mut store);
        assert!(!CONFIG.exists(&store));

        // safe to remove 2 times
        CONFIG.remove(&mut store);
        assert!(!CONFIG.exists(&store));
    }

    #[test]
    fn isolated_reads() {
        let mut store = MockStorage::new();

        let cfg = Config {
            owner: "admin".to_string(),
            max_tokens: 1234,
        };
        CONFIG.save(&mut store, &cfg).unwrap();

        let reader = Item::<Config>::new("config");
        assert_eq!(cfg, reader.load(&store).unwrap());

        let other_reader = Item::<Config>::new("config2");
        assert_eq!(other_reader.may_load(&store).unwrap(), None);
    }

    #[test]
    fn update_success() {
        let mut store = MockStorage::new();

        let cfg = Config {
            owner: "admin".to_string(),
            max_tokens: 1234,
        };
        CONFIG.save(&mut store, &cfg).unwrap();

        let output = CONFIG.update(&mut store, |mut c| -> StdResult<_> {
            c.max_tokens *= 2;
            Ok(c)
        });
        let expected = Config {
            owner: "admin".to_string(),
            max_tokens: 2468,
        };
        assert_eq!(output.unwrap(), expected);
        assert_eq!(CONFIG.load(&store).unwrap(), expected);
    }

    #[test]
    fn update_can_change_variable_from_outer_scope() {
        let mut store = MockStorage::new();
        let cfg = Config {
            owner: "admin".to_string(),
            max_tokens: 1234,
        };
        CONFIG.save(&mut store, &cfg).unwrap();

        let mut old_max_tokens = 0i32;
        CONFIG
            .update(&mut store, |mut c| -> StdResult<_> {
                old_max_tokens = c.max_tokens;
                c.max_tokens *= 2;
                Ok(c)
            })
            .unwrap();
        assert_eq!(old_max_tokens, 1234);
    }

    #[test]
    fn update_does_not_change_data_on_error() {
        let mut store = MockStorage::new();

        let cfg = Config {
            owner: "admin".to_string(),
            max_tokens: 1234,
        };
        CONFIG.save(&mut store, &cfg).unwrap();

        let output = CONFIG.update(&mut store, |_c| {
            Err(StdError::overflow(OverflowError::new(
                OverflowOperation::Sub,
            )))
        });
        match output.unwrap_err() {
            StdError::Overflow { .. } => {}
            err => panic!("Unexpected error: {:?}", err),
        }
        assert_eq!(CONFIG.load(&store).unwrap(), cfg);
    }

    #[test]
    fn update_supports_custom_errors() {
        #[derive(Debug)]
        enum MyError {
            Std(StdError),
            Foo,
        }

        impl From<StdError> for MyError {
            fn from(original: StdError) -> MyError {
                MyError::Std(original)
            }
        }

        let mut store = MockStorage::new();

        let cfg = Config {
            owner: "admin".to_string(),
            max_tokens: 1234,
        };
        CONFIG.save(&mut store, &cfg).unwrap();

        let res = CONFIG.update(&mut store, |mut c| {
            if c.max_tokens > 5000 {
                return Err(MyError::Foo);
            }
            if c.max_tokens > 20 {
                return Err(StdError::generic_err("broken stuff").into()); // Uses Into to convert StdError to MyError
            }
            if c.max_tokens > 10 {
                to_json_vec(&c)?; // Uses From to convert StdError to MyError
            }
            c.max_tokens += 20;
            Ok(c)
        });
        match res.unwrap_err() {
            MyError::Std(StdError::GenericErr { .. }) => {}
            err => panic!("Unexpected error: {:?}", err),
        }
        assert_eq!(CONFIG.load(&store).unwrap(), cfg);
    }

    #[test]
    fn readme_works() -> StdResult<()> {
        let mut store = MockStorage::new();

        // may_load returns Option<T>, so None if data is missing
        // load returns T and Err(StdError::NotFound{}) if data is missing
        let empty = CONFIG.may_load(&store)?;
        assert_eq!(None, empty);
        let cfg = Config {
            owner: "admin".to_string(),
            max_tokens: 1234,
        };
        CONFIG.save(&mut store, &cfg)?;
        let loaded = CONFIG.load(&store)?;
        assert_eq!(cfg, loaded);

        // update an item with a closure (includes read and write)
        // returns the newly saved value
        let output = CONFIG.update(&mut store, |mut c| -> StdResult<_> {
            c.max_tokens *= 2;
            Ok(c)
        })?;
        assert_eq!(2468, output.max_tokens);

        // you can error in an update and nothing is saved
        let failed = CONFIG.update(&mut store, |_| -> StdResult<_> {
            Err(StdError::generic_err("failure mode"))
        });
        assert!(failed.is_err());

        // loading data will show the first update was saved
        let loaded = CONFIG.load(&store)?;
        let expected = Config {
            owner: "admin".to_string(),
            max_tokens: 2468,
        };
        assert_eq!(expected, loaded);

        // we can remove data as well
        CONFIG.remove(&mut store);
        let empty = CONFIG.may_load(&store)?;
        assert_eq!(None, empty);

        Ok(())
    }
}
