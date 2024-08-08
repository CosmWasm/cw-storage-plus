use cosmwasm_std::{
    testing::{mock_env, MockStorage},
    Addr,
};
use cw_storage_plus::{IndexedMap, MultiIndex, SnapshotItem, UniqueIndex};
use serde::{Deserialize, Serialize};

#[test]
fn foo() {
    let mut storage = MockStorage::new();
    let env = mock_env();

    let foo = SnapshotItem::<u64>::new("f", "f1", "f2", cw_storage_plus::Strategy::EveryBlock);

    for i in 0..10 {
        foo.save(&mut storage, &i, env.block.height + i * 10)
            .unwrap();
        foo.add_checkpoint(&mut storage, env.block.height + 1 + i * 10)
            .unwrap();
    }

    assert_eq!(
        foo.may_load_at_height(&storage, env.block.height + 10)
            .unwrap(),
        Some(1)
    );
}
