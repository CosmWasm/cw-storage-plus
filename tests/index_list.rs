#[cfg(all(test, feature = "iterator", feature = "macro"))]
mod test {
    use cosmwasm_std::testing::MockStorage;
    use cw_storage_macro::index_list;
    use cw_storage_proto::{IndexedMap, MultiIndex, UniqueIndex};

    #[test]
    fn index_list_compiles() {
        #[derive(prost::Message, Clone, PartialEq)]
        struct TestStruct {
            #[prost(uint64, tag = "1")]
            id: u64,
            #[prost(uint32, tag = "2")]
            id2: u32,
            #[prost(string, tag = "3")]
            addr: String,
        }

        #[index_list(TestStruct)]
        struct TestIndexes<'a> {
            id: MultiIndex<'a, u32, TestStruct, u64>,
            addr: UniqueIndex<'a, String, TestStruct>,
        }

        let _: IndexedMap<u64, TestStruct, TestIndexes> = IndexedMap::new(
            "t",
            TestIndexes {
                id: MultiIndex::new(|_pk, t| t.id2, "t", "t_id2"),
                addr: UniqueIndex::new(|t| t.addr.clone(), "t_addr"),
            },
        );
    }

    #[test]
    fn index_list_works() {
        #[derive(prost::Message, Clone, PartialEq)]
        struct TestStruct {
            #[prost(uint64, tag = "1")]
            id: u64,
            #[prost(uint32, tag = "2")]
            id2: u32,
            #[prost(string, tag = "3")]
            addr: String,
        }

        #[index_list(TestStruct)]
        struct TestIndexes<'a> {
            id: MultiIndex<'a, u32, TestStruct, u64>,
            addr: UniqueIndex<'a, String, TestStruct>,
        }

        let mut storage = MockStorage::new();
        let idm: IndexedMap<u64, TestStruct, TestIndexes> = IndexedMap::new(
            "t",
            TestIndexes {
                id: MultiIndex::new(|_pk, t| t.id2, "t", "t_2"),
                addr: UniqueIndex::new(|t| t.addr.clone(), "t_addr"),
            },
        );

        idm.save(
            &mut storage,
            0,
            &TestStruct {
                id: 0,
                id2: 100,
                addr: "1".into(),
            },
        )
        .unwrap();

        assert_eq!(
            idm.load(&storage, 0).unwrap(),
            TestStruct {
                id: 0,
                id2: 100,
                addr: "1".into(),
            }
        );
    }
}
