#[cfg(all(test, feature = "iterator", feature = "macro"))]
mod test {
    use cosmwasm_schema::cw_prost;
    use cosmwasm_std::{testing::MockStorage, Addr};
    use cw_storage_macro::index_list;
    use cw_storage_plus::{IndexedMap, MultiIndex, UniqueIndex};

    #[test]
    fn index_list_compiles() {
        #[cw_prost]
        struct TestStruct {
            #[prost(uint64, tag = "1")]
            id: u64,
            #[prost(uint32, tag = "2")]
            id2: u32,
            #[prost(message, tag = "3")]
            addr: Addr,
        }

        #[index_list(TestStruct)]
        struct TestIndexes<'a> {
            id: MultiIndex<'a, u32, TestStruct, u64>,
            addr: UniqueIndex<'a, Addr, TestStruct, ()>,
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
        #[cw_prost]
        struct TestStruct {
            #[prost(uint64, tag = "1")]
            id: u64,
            #[prost(uint32, tag = "2")]
            id2: u32,
            #[prost(message, tag = "3")]
            addr: Addr,
        }

        #[index_list(TestStruct)]
        struct TestIndexes<'a> {
            id: MultiIndex<'a, u32, TestStruct, u64>,
            addr: UniqueIndex<'a, Addr, TestStruct, ()>,
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
                addr: Addr::unchecked("1"),
            },
        )
        .unwrap();

        assert_eq!(
            idm.load(&storage, 0).unwrap(),
            TestStruct {
                id: 0,
                id2: 100,
                addr: Addr::unchecked("1"),
            }
        );
    }
}
