use std::array::TryFromSliceError;
use std::convert::TryInto;

use cosmwasm_std::{Addr, Int128, Int64, StdError, StdResult, Uint128, Uint64};

use crate::int_key::IntKey;

pub trait KeyDeserialize {
    type Output: Sized;

    /// The number of key elements is used for the deserialization of compound keys.
    /// It should be equal to PrimaryKey::key().len()
    const KEY_ELEMS: u16;

    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output>;

    fn from_slice(value: &[u8]) -> StdResult<Self::Output> {
        Self::from_vec(value.to_vec())
    }
}

impl KeyDeserialize for () {
    type Output = ();

    const KEY_ELEMS: u16 = 0;

    #[inline(always)]
    fn from_vec(_value: Vec<u8>) -> StdResult<Self::Output> {
        Ok(())
    }
}

impl KeyDeserialize for Vec<u8> {
    type Output = Vec<u8>;

    const KEY_ELEMS: u16 = 1;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        Ok(value)
    }
}

impl KeyDeserialize for &Vec<u8> {
    type Output = Vec<u8>;

    const KEY_ELEMS: u16 = 1;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        Ok(value)
    }
}

impl KeyDeserialize for &[u8] {
    type Output = Vec<u8>;

    const KEY_ELEMS: u16 = 1;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        Ok(value)
    }
}

impl<const N: usize> KeyDeserialize for [u8; N] {
    type Output = [u8; N];

    const KEY_ELEMS: u16 = 1;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        <[u8; N]>::try_from(value).map_err(|v: Vec<_>| {
            StdError::msg(format!(
                "invalid_data_size: expected {}, actual {}",
                N,
                v.len()
            ))
        })
    }
}

impl<const N: usize> KeyDeserialize for &[u8; N] {
    type Output = [u8; N];

    const KEY_ELEMS: u16 = 1;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        <[u8; N]>::from_vec(value)
    }
}

impl KeyDeserialize for String {
    type Output = String;

    const KEY_ELEMS: u16 = 1;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        Ok(String::from_utf8(value)?)
    }
}

impl KeyDeserialize for &String {
    type Output = String;

    const KEY_ELEMS: u16 = 1;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        Self::Output::from_vec(value)
    }
}

impl KeyDeserialize for &str {
    type Output = String;

    const KEY_ELEMS: u16 = 1;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        Self::Output::from_vec(value)
    }
}

impl KeyDeserialize for Addr {
    type Output = Addr;

    const KEY_ELEMS: u16 = 1;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        Ok(Addr::unchecked(String::from_vec(value)?))
    }
}

impl KeyDeserialize for &Addr {
    type Output = Addr;

    const KEY_ELEMS: u16 = 1;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        Self::Output::from_vec(value)
    }
}

macro_rules! integer_de {
    (for $($t:ty),+) => {
        $(impl KeyDeserialize for $t {
            type Output = $t;

            const KEY_ELEMS: u16 = 1;

            #[inline(always)]
            fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
                Ok(<$t>::from_cw_bytes(value.as_slice().try_into()
                    .map_err(|err: TryFromSliceError| StdError::msg(err.to_string()))?))
            }
        })*
    }
}

integer_de!(for i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, Uint64, Uint128, Int64, Int128);

fn parse_length(value: &[u8]) -> StdResult<usize> {
    Ok(u16::from_be_bytes(
        value
            .try_into()
            .map_err(|_| StdError::msg("Could not read 2 byte length"))?,
    )
    .into())
}

/// Splits the first key from the value based on the provided number of key elements.
/// The return value is ordered as (first_key, remainder).
///
fn split_first_key(key_elems: u16, value: &[u8]) -> StdResult<(Vec<u8>, &[u8])> {
    let mut index = 0;
    let mut first_key = Vec::new();

    // Iterate over the sub keys
    for i in 0..key_elems {
        let len_slice = &value[index..index + 2];
        index += 2;
        let is_last_key = i == key_elems - 1;

        if !is_last_key {
            first_key.extend_from_slice(len_slice);
        }

        let subkey_len = parse_length(len_slice)?;
        first_key.extend_from_slice(&value[index..index + subkey_len]);
        index += subkey_len;
    }

    let remainder = &value[index..];
    Ok((first_key, remainder))
}

impl<T: KeyDeserialize, U: KeyDeserialize> KeyDeserialize for (T, U) {
    type Output = (T::Output, U::Output);

    const KEY_ELEMS: u16 = T::KEY_ELEMS + U::KEY_ELEMS;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        let (t, u) = split_first_key(T::KEY_ELEMS, value.as_ref())?;
        Ok((T::from_vec(t)?, U::from_vec(u.to_vec())?))
    }
}

impl<T: KeyDeserialize, U: KeyDeserialize, V: KeyDeserialize> KeyDeserialize for (T, U, V) {
    type Output = (T::Output, U::Output, V::Output);

    const KEY_ELEMS: u16 = T::KEY_ELEMS + U::KEY_ELEMS + V::KEY_ELEMS;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        let (t, remainder) = split_first_key(T::KEY_ELEMS, value.as_ref())?;
        let (u, v) = split_first_key(U::KEY_ELEMS, remainder)?;
        Ok((T::from_vec(t)?, U::from_vec(u)?, V::from_vec(v.to_vec())?))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::PrimaryKey;

    const BYTES: &[u8] = b"Hello";
    const STRING: &str = "Hello";

    #[test]
    #[allow(clippy::unit_cmp)]
    fn deserialize_empty_works() {
        assert_eq!(<()>::from_slice(BYTES).unwrap(), ());
    }

    #[test]
    fn deserialize_bytes_works() {
        assert_eq!(<Vec<u8>>::from_slice(BYTES).unwrap(), BYTES);
        assert_eq!(<&Vec<u8>>::from_slice(BYTES).unwrap(), BYTES);
        assert_eq!(<&[u8]>::from_slice(BYTES).unwrap(), BYTES);
        assert_eq!(<[u8; 5]>::from_slice(BYTES).unwrap(), BYTES);
        assert_eq!(<&[u8; 5]>::from_slice(BYTES).unwrap(), BYTES);
    }

    #[test]
    fn deserialize_string_works() {
        assert_eq!(<String>::from_slice(BYTES).unwrap(), STRING);
        assert_eq!(<&String>::from_slice(BYTES).unwrap(), STRING);
        assert_eq!(<&str>::from_slice(BYTES).unwrap(), STRING);
    }

    #[test]
    fn deserialize_broken_string_errs() {
        assert_eq!(
            "kind: Parsing, error: incomplete utf-8 byte sequence from index 0",
            <String>::from_slice(b"\xc3").unwrap_err().to_string()
        );
    }

    #[test]
    fn deserialize_addr_works() {
        assert_eq!(<Addr>::from_slice(BYTES).unwrap(), Addr::unchecked(STRING));
        assert_eq!(<&Addr>::from_slice(BYTES).unwrap(), Addr::unchecked(STRING));
    }

    #[test]
    fn deserialize_broken_addr_errs() {
        assert_eq!(
            "kind: Parsing, error: incomplete utf-8 byte sequence from index 0",
            <Addr>::from_slice(b"\xc3").unwrap_err().to_string()
        );
    }

    #[test]
    fn deserialize_naked_integer_works() {
        assert_eq!(u8::from_slice(&[1]).unwrap(), 1u8);
        assert_eq!(i8::from_slice(&[127]).unwrap(), -1i8);
        assert_eq!(i8::from_slice(&[128]).unwrap(), 0i8);

        assert_eq!(u16::from_slice(&[1, 0]).unwrap(), 256u16);
        assert_eq!(i16::from_slice(&[128, 0]).unwrap(), 0i16);
        assert_eq!(i16::from_slice(&[127, 255]).unwrap(), -1i16);

        assert_eq!(u32::from_slice(&[1, 0, 0, 0]).unwrap(), 16777216u32);
        assert_eq!(i32::from_slice(&[128, 0, 0, 0]).unwrap(), 0i32);
        assert_eq!(i32::from_slice(&[127, 255, 255, 255]).unwrap(), -1i32);

        assert_eq!(
            u64::from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]).unwrap(),
            72057594037927936u64
        );
        assert_eq!(i64::from_slice(&[128, 0, 0, 0, 0, 0, 0, 0]).unwrap(), 0i64);
        assert_eq!(
            i64::from_slice(&[127, 255, 255, 255, 255, 255, 255, 255]).unwrap(),
            -1i64
        );

        assert_eq!(
            u128::from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap(),
            1329227995784915872903807060280344576u128
        );
        assert_eq!(
            i128::from_slice(&[128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap(),
            0i128
        );
        assert_eq!(
            i128::from_slice(&[
                127, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255
            ])
            .unwrap(),
            -1i128
        );
        assert_eq!(
            i128::from_slice(&[
                255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255
            ])
            .unwrap(),
            170141183460469231731687303715884105727i128,
        );
    }

    #[test]
    fn deserialize_tuple_works() {
        assert_eq!(
            <(&[u8], &str)>::from_slice((BYTES, STRING).joined_key().as_slice()).unwrap(),
            (BYTES.to_vec(), STRING.to_string())
        );
    }

    #[test]
    fn deserialize_tuple_of_tuples_works() {
        assert_eq!(
            <((&[u8], &str), (&[u8], &str))>::from_slice(
                ((BYTES, STRING), (BYTES, STRING)).joined_key().as_slice()
            )
            .unwrap(),
            (
                (BYTES.to_vec(), STRING.to_string()),
                (BYTES.to_vec(), STRING.to_string())
            )
        );
    }

    #[test]
    fn deserialize_tuple_of_triples_works() {
        assert_eq!(
            <((&[u8], &str, u32), (&[u8], &str, u16))>::from_slice(
                ((BYTES, STRING, 1234u32), (BYTES, STRING, 567u16))
                    .joined_key()
                    .as_slice()
            )
            .unwrap(),
            (
                (BYTES.to_vec(), STRING.to_string(), 1234),
                (BYTES.to_vec(), STRING.to_string(), 567)
            )
        );
    }

    #[test]
    fn deserialize_triple_of_tuples_works() {
        assert_eq!(
            <((u32, &str), (&str, &[u8]), (i32, i32))>::from_slice(
                ((1234u32, STRING), (STRING, BYTES), (1234i32, 567i32))
                    .joined_key()
                    .as_slice()
            )
            .unwrap(),
            (
                (1234, STRING.to_string()),
                (STRING.to_string(), BYTES.to_vec()),
                (1234, 567)
            )
        );
    }

    #[test]
    fn deserialize_triple_of_triples_works() {
        assert_eq!(
            <((u32, &str, &str), (&str, &[u8], u8), (i32, u8, i32))>::from_slice(
                (
                    (1234u32, STRING, STRING),
                    (STRING, BYTES, 123u8),
                    (4567i32, 89u8, 10i32)
                )
                    .joined_key()
                    .as_slice()
            )
            .unwrap(),
            (
                (1234, STRING.to_string(), STRING.to_string()),
                (STRING.to_string(), BYTES.to_vec(), 123),
                (4567, 89, 10)
            )
        );
    }

    #[test]
    fn deserialize_triple_works() {
        assert_eq!(
            <(&[u8], u32, &str)>::from_slice((BYTES, 1234u32, STRING).joined_key().as_slice())
                .unwrap(),
            (BYTES.to_vec(), 1234, STRING.to_string())
        );
    }
}
