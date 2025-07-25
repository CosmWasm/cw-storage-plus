use cosmwasm_std::storage_keys::namespace_with_key;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::marker::PhantomData;

use crate::helpers::not_found_object_info;
use cosmwasm_std::{from_json, to_json_vec, StdError, StdResult, Storage};
use std::ops::Deref;

#[derive(Debug, Clone)]
pub struct Path<T>
where
    T: Serialize + DeserializeOwned,
{
    /// all namespaces prefixes and concatenated with the key
    pub(crate) storage_key: Vec<u8>,
    // see https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-type-parameters for why this is needed
    data: PhantomData<T>,
}

impl<T> Deref for Path<T>
where
    T: Serialize + DeserializeOwned,
{
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.storage_key
    }
}

impl<T> Path<T>
where
    T: Serialize + DeserializeOwned,
{
    pub fn new(namespace: &[u8], keys: &[&[u8]]) -> Self {
        let l = keys.len();

        // Combine namespace and all but last keys.
        // This is a single vector allocation with references as elements.
        let calculated_len = 1 + keys.len() - 1;
        let mut combined: Vec<&[u8]> = Vec::with_capacity(calculated_len);
        combined.push(namespace);
        combined.extend(keys[0..l - 1].iter());
        debug_assert_eq!(calculated_len, combined.len()); // as long as we calculate correctly, we don't need to reallocate
        let storage_key = namespace_with_key(&combined, keys[l - 1]);
        Path {
            storage_key,
            data: PhantomData,
        }
    }

    /// save will serialize the model and store, returns an error on serialization issues
    pub fn save(&self, store: &mut dyn Storage, data: &T) -> StdResult<()> {
        store.set(&self.storage_key, &to_json_vec(data)?);
        Ok(())
    }

    pub fn remove(&self, store: &mut dyn Storage) {
        store.remove(&self.storage_key);
    }

    /// load will return an error if no data is set at the given key, or on parse error
    pub fn load(&self, store: &dyn Storage) -> StdResult<T> {
        if let Some(value) = store.get(&self.storage_key) {
            from_json(value)
        } else {
            let object_info = not_found_object_info::<T>(&self.storage_key);
            Err(StdError::msg(format!("{object_info} not found")))
        }
    }

    /// may_load will parse the data stored at the key if present, returns Ok(None) if no data there.
    /// returns an error on issues parsing
    pub fn may_load(&self, store: &dyn Storage) -> StdResult<Option<T>> {
        let value = store.get(&self.storage_key);
        value.map(|v| from_json(v)).transpose()
    }

    /// has returns true or false if any data is at this key, without parsing or interpreting the
    /// contents. It will returns true for an length-0 byte array (Some(b"")), if you somehow manage to set that.
    pub fn has(&self, store: &dyn Storage) -> bool {
        store.get(&self.storage_key).is_some()
    }

    /// Loads the data, perform the specified action, and store the result
    /// in the database. This is shorthand for some common sequences, which may be useful.
    ///
    /// If the data exists, `action(Some(value))` is called. Otherwise, `action(None)` is called.
    pub fn update<A, E>(&self, store: &mut dyn Storage, action: A) -> Result<T, E>
    where
        A: FnOnce(Option<T>) -> Result<T, E>,
        E: From<StdError>,
    {
        let input = self.may_load(store)?;
        let output = action(input)?;
        self.save(store, &output)?;
        Ok(output)
    }
}
