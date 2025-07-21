//! This module is an implementation of a namespacing scheme described
//! in <https://github.com/webmaster128/key-namespacing#length-prefixed-keys>
//!
//! Everything in this file is only responsible for building such keys
//! and is in no way specific to any kind of storage.

use std::any::type_name;

use cosmwasm_std::{
    to_json_vec, Addr, Binary, ContractResult, CustomQuery, QuerierWrapper, QueryRequest, StdError,
    StdResult, SystemResult, WasmQuery,
};

/// Use this in Map/SnapshotMap/etc when you want to provide a QueryRaw helper.
/// This is similar to `querier.query(WasmQuery::Raw {})`, except it does NOT parse the
/// result, but return a possibly empty Binary to be handled by the calling code.
/// That is essential to handle b"" as None.
pub(crate) fn query_raw<Q: CustomQuery>(
    querier: &QuerierWrapper<Q>,
    contract_addr: Addr,
    key: Binary,
) -> StdResult<Binary> {
    let request: QueryRequest<Q> = WasmQuery::Raw {
        contract_addr: contract_addr.into(),
        key,
    }
    .into();

    let raw = to_json_vec(&request).map_err(|serialize_err| {
        StdError::msg(format!("Serializing QueryRequest: {serialize_err}"))
    })?;
    match querier.raw_query(&raw) {
        SystemResult::Err(system_err) => {
            Err(StdError::msg(format!("Querier system error: {system_err}")))
        }
        SystemResult::Ok(ContractResult::Err(contract_err)) => Err(StdError::msg(format!(
            "Querier contract error: {contract_err}"
        ))),
        SystemResult::Ok(ContractResult::Ok(value)) => Ok(value),
    }
}

/// Returns a debug identifier to explain what was not found
pub(crate) fn not_found_object_info<T>(key: &[u8]) -> String {
    let type_name = type_name::<T>();
    format!("type: {type_name}; key: {key:02X?}")
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::Uint128;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Person {
        pub name: String,
        pub age: i32,
    }

    #[test]
    fn not_found_object_info_works() {
        assert_eq!(
            not_found_object_info::<Person>(&[0xaa, 0xBB]),
            "type: cw_storage_plus::helpers::test::Person; key: [AA, BB]"
        );
        assert_eq!(
            not_found_object_info::<Person>(&[]),
            "type: cw_storage_plus::helpers::test::Person; key: []"
        );
        assert_eq!(
            not_found_object_info::<Uint128>(b"foo"),
            "type: cosmwasm_std::math::uint128::Uint128; key: [66, 6F, 6F]"
        );
    }
}
