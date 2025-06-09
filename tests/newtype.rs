#[cfg(all(test, feature = "iterator", feature = "macro"))]
mod test {
    use std::ops::Deref;

    use cosmwasm_std::{testing::MockStorage, Addr, Uint128};
    use cw_storage_macro::NewTypeKey;
    use cw_storage_plus::Map;
    use derive_more::with_trait::Display;
    use serde::{Deserialize, Serialize};

    #[test]
    fn newtype_compiles() {
        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, NewTypeKey)]
        struct TestKey(u64);

        let _ = TestKey(100);
    }

    #[test]
    fn newtype_works() {
        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, NewTypeKey, Display)]
        struct TestKeyU8(u8);
        impl Deref for TestKeyU8 {
            type Target = u8;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, NewTypeKey, Display)]
        struct TestKeyU64(u64);
        impl Deref for TestKeyU64 {
            type Target = u64;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, NewTypeKey, Display)]
        struct TestKeyU128(u128);
        impl Deref for TestKeyU128 {
            type Target = u128;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, NewTypeKey, Display)]
        struct TestKeyUint128(Uint128);
        impl Deref for TestKeyUint128 {
            type Target = Uint128;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, NewTypeKey, Display)]
        struct TestKeyString(String);
        impl Deref for TestKeyString {
            type Target = String;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, NewTypeKey, Display)]
        struct TestKeyAddr(Addr);
        impl Deref for TestKeyAddr {
            type Target = Addr;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        fn run<'a, T, U>(key: T, map: Map<T, String>, expected_str: &str)
        where
            T: cw_storage_plus::PrimaryKey<'a> + Display + Deref<Target = U>,
            U: cw_storage_plus::PrimaryKey<'a>,
        {
            let mut storage = MockStorage::new();

            // they should serialize to the same string
            assert_eq!(key.to_string(), expected_str);

            // they should have the same underlying key
            let inner_key: &U = key.deref();
            assert_eq!(inner_key.joined_key(), key.joined_key());

            // the newtype wrapper should work for maps
            let value = "value".to_string();
            map.save(&mut storage, key.clone(), &value).unwrap();
            assert_eq!(map.load(&storage, key.clone()).unwrap(), value);
        }

        run::<_, u8>(TestKeyU8(1u8), Map::new("map-1"), "1");
        run::<_, u64>(TestKeyU64(2u64), Map::new("map-2"), "2");
        run::<_, u128>(TestKeyU128(3u128), Map::new("map-3"), "3");
        run::<_, Uint128>(TestKeyUint128(Uint128::new(4u128)), Map::new("map-4"), "4");
        run::<_, String>(
            TestKeyString("my_key".to_string()),
            Map::new("map-5"),
            "my_key",
        );
        run::<_, Addr>(
            TestKeyAddr(Addr::unchecked("my_addr".to_string())),
            Map::new("map-6"),
            "my_addr",
        );
    }
}
