use std::any::type_name;

use cosmwasm_std::{Binary, StdError, StdResult};

/// Uses protobuf (instead of serde-json-wasm) to serialize data to bytes.
/// Drop-in replacement for `cosmwasm_std::to_vec`.
///
/// NOTE: Protobuf encoding always succeeds, so we technically don't need to
/// return an result type here. However, `cosmwasm_std::to_vec` returns `StdResult`,
/// so to keep diff small, we also return `StdResult`.
pub fn to_vec<T: prost::Message>(data: &T) -> StdResult<Vec<u8>> {
    let bytes = T::encode_to_vec(data);

    // We need to do a bit of hacking here. There are some cases where protobuf
    // serializes a data to an empty byte array, which includes:
    // - a single number zero (`0u32`, `0u64`, etc.)
    // - a single boolean `false`
    // - a struct where all fields are all optional and `None`
    //
    // However, CosmWasm doesn't allow storing a key-value pair where the value
    // is empty:
    // https://github.com/CosmWasm/cosmwasm/blob/v1.1.8/packages/std/src/storage.rs#L30
    //
    // To work this around this, if a data serializes to an empty byte array,
    // we simply return a single zero byte `vec![0u8]`.
    //
    // There is no way a protobuf encoding is only a single byte, so there's no
    // ambiguity with what this mean.
    //
    // When loading, if a value is a single zero byte, we interpret it as if
    // the value is an empty byte array.
    if bytes.is_empty() {
        return Ok(vec![0]);
    }

    Ok(bytes)
}

/// Uses protobuf (instead of serde-json-wasm) to serialize data to bytes, and
/// cast the result to `cosmwasm_std::Binary` type.
/// Drop-in replacement for `cosmwasm_std::to_binary`.
pub fn to_binary<T: prost::Message>(data: &T) -> StdResult<Binary> {
    to_vec(data).map(Binary)
}

/// Uses protobuf (instead of serde-json-wasm) to deserialize bytes to data.
/// Drop-in replacement for `cosmwasm_std::from_slice`.
pub fn from_slice<T: prost::Message + Default>(bytes: &[u8]) -> StdResult<T> {
    // See the comments in `to_vec` on why we do this here.
    if bytes == [0] {
        return from_slice(&[]);
    }

    T::decode(bytes).map_err(|err| StdError::parse_err(type_name::<T>(), err.to_string()))
}
