use cosmwasm_std::{StdResult, Storage};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{KeyDeserialize, Prefixer, PrimaryKey};

use super::{Snapshot, SnapshotStrategy};

/// A SnapshotStrategy that takes a snapshot only if at least the specified interval has passed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IntervalStrategy {
    /// The interval to archive snapshots at. If the time or number of blocks since the last changelog
    /// entry is greater than this interval, a new snapshot will be created.
    pub interval: u64,
}

impl IntervalStrategy {
    /// Create a new IntervalStrategy with the given interval.
    pub const fn new(interval: u64) -> Self {
        Self { interval }
    }
}

impl<'a, K, T> SnapshotStrategy<'a, K, T> for IntervalStrategy
where
    T: Serialize + DeserializeOwned + Clone,
    K: PrimaryKey<'a> + Prefixer<'a> + KeyDeserialize,
{
    fn assert_checkpointed(
        &self,
        _store: &dyn Storage,
        _snapshot: &Snapshot<K, T, Self>,
        _height: u64,
    ) -> StdResult<()> {
        Ok(())
    }

    fn should_archive(
        &self,
        store: &dyn Storage,
        snapshot: &Snapshot<'a, K, T, Self>,
        key: &K,
        height: u64,
    ) -> StdResult<bool> {
        let last_height = height.saturating_sub(self.interval);
        let change_since_interval = snapshot.may_load_at_height(store, key.clone(), last_height)?;

        Ok(change_since_interval.is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::MockStorage;

    type TestSnapshot = Snapshot<'static, &'static str, u64, IntervalStrategy>;
    const INTERVAL_5: TestSnapshot = Snapshot::new(
        "interval_5__check",
        "interval_5__change",
        IntervalStrategy::new(5),
    );

    const DUMMY_KEY: &str = "dummy";

    #[test]
    fn should_archive() {
        let mut store = MockStorage::new();

        // Should archive first save since there is no previous changelog entry.
        assert_eq!(INTERVAL_5.should_archive(&store, &DUMMY_KEY, 0), Ok(true));

        // Store changelog entry
        INTERVAL_5
            .write_changelog(&mut store, DUMMY_KEY, 0, None)
            .unwrap();

        // Should not archive again
        assert_eq!(INTERVAL_5.should_archive(&store, &DUMMY_KEY, 0), Ok(false));

        // Should archive once interval has passed
        assert_eq!(INTERVAL_5.should_archive(&store, &DUMMY_KEY, 6), Ok(true));

        // Store changelog entry
        INTERVAL_5
            .write_changelog(&mut store, DUMMY_KEY, 6, None)
            .unwrap();

        // Should not archive again
        assert_eq!(INTERVAL_5.should_archive(&store, &DUMMY_KEY, 6), Ok(false));

        // Should not archive before interval
        assert_eq!(
            INTERVAL_5.should_archive(&store, &DUMMY_KEY, 6 + 5),
            Ok(false)
        );

        // Should archive once interval has passed
        assert_eq!(
            INTERVAL_5.should_archive(&store, &DUMMY_KEY, 6 + 5 + 1),
            Ok(true)
        );
    }
}
