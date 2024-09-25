#[cfg(all(test, feature = "iterator", feature = "macro"))]
mod test {
    use cosmwasm_std::{testing::MockStorage, Addr, Uint128};
    use cw_storage_macro::NewTypeKey;
    use cw_storage_plus::Map;
    use derive_more::Display;
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

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, NewTypeKey, Display)]
        struct TestKeyU64(u64);

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, NewTypeKey, Display)]
        struct TestKeyU128(u128);

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, NewTypeKey, Display)]
        struct TestKeyUint128(Uint128);

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, NewTypeKey, Display)]
        struct TestKeyString(String);

        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, NewTypeKey, Display)]
        struct TestKeyAddr(Addr);

        fn run<'a, T: cw_storage_plus::PrimaryKey<'a> + Display>(
            key: T,
            map: Map<T, String>,
            expected_str: &str,
        ) {
            let mut storage = MockStorage::new();

            assert_eq!(key.to_string(), expected_str);

            let value = "value".to_string();

            map.save(&mut storage, key.clone(), &value).unwrap();

            assert_eq!(map.load(&storage, key).unwrap(), value);
        }

        run(TestKeyU8(1u8), Map::new("map-1"), "1");
        run(TestKeyU64(2u64), Map::new("map-2"), "2");
        run(TestKeyU128(3u128), Map::new("map-3"), "3");
        run(TestKeyUint128(Uint128::new(4u128)), Map::new("map-4"), "4");
        run(
            TestKeyString("my_key".to_string()),
            Map::new("map-5"),
            "my_key",
        );
        run(
            TestKeyAddr(Addr::unchecked("my_addr".to_string())),
            Map::new("map-6"),
            "my_addr",
        );
    }
}
