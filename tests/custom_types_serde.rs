use cosmwasm_std::{Addr, StdError, StdResult};
use cw_storage_plus::{split_first_key, Key, KeyDeserialize, Prefixer, PrimaryKey};
use std::u8;

/// This file an example of the `PrimaryKey` and `KeyDeserialize` implementation for a custom type
/// `Denom` which can be either a native token or a cw20 token address.
///
/// The idea is to store the Denom in the storage as a composite key with 2 elements:
/// 1. The prefix which is either `NATIVE_PREFIX` or `CW20_PREFIX` to differentiate between the
/// two types on a raw bytes level
/// 2. The value which is either the native token name or the cw20 token address

/// Define a custom type which is Denom that can be either Native or Cw20 token address
#[derive(Clone, Debug, PartialEq, Eq)]
enum Denom {
    Native(String),
    Cw20(Addr),
}

const NATIVE_PREFIX: u8 = 1;
const CW20_PREFIX: u8 = 2;

impl PrimaryKey<'_> for Denom {
    type Prefix = u8;
    type SubPrefix = ();
    type Suffix = String;
    type SuperSuffix = Self;

    fn key(&self) -> Vec<Key> {
        let (prefix, value) = match self {
            Denom::Native(name) => (NATIVE_PREFIX, name.as_bytes()),
            Denom::Cw20(addr) => (CW20_PREFIX, addr.as_bytes()),
        };
        vec![Key::Val8([prefix]), Key::Ref(value)]
    }
}

impl Prefixer<'_> for Denom {
    fn prefix(&self) -> Vec<Key> {
        let (prefix, value) = match self {
            Denom::Native(name) => (NATIVE_PREFIX.prefix(), name.prefix()),
            Denom::Cw20(addr) => (CW20_PREFIX.prefix(), addr.prefix()),
        };

        let mut result: Vec<Key> = vec![];
        result.extend(prefix);
        result.extend(value);
        result
    }
}

impl KeyDeserialize for Denom {
    type Output = Self;

    const KEY_ELEMS: u16 = 2;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        let (prefix, value) = split_first_key(Self::KEY_ELEMS, value.as_ref())?;
        let value = value.to_vec();

        match u8::from_vec(prefix)? {
            NATIVE_PREFIX => Ok(Denom::Native(String::from_vec(value)?)),
            CW20_PREFIX => Ok(Denom::Cw20(Addr::from_vec(value)?)),
            _ => Err(StdError::generic_err("Invalid prefix")),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::Denom;
    use cosmwasm_std::testing::MockStorage;
    use cosmwasm_std::{Addr, Uint64};
    use cw_storage_plus::Map;

    #[test]
    fn round_trip_tests() {
        let test_data = vec![
            Denom::Native("cosmos".to_string()),
            Denom::Native("some_long_native_value_with_high_precise".to_string()),
            Denom::Cw20(Addr::unchecked("contract1")),
            Denom::Cw20(Addr::unchecked(
                "cosmos1p7d8mnjttcszv34pk2a5yyug3474mhffasa7tg",
            )),
        ];

        for denom in test_data {
            verify_map_serde(denom);
        }
    }

    fn verify_map_serde(denom: Denom) {
        let mut storage = MockStorage::new();
        let map: Map<Denom, Uint64> = Map::new("denom_map");
        let mock_value = Uint64::from(123u64);

        map.save(&mut storage, denom.clone(), &mock_value).unwrap();

        assert!(map.has(&storage, denom.clone()), "key should exist");

        let value = map.load(&storage, denom).unwrap();
        assert_eq!(value, mock_value, "value should match");
    }
}
