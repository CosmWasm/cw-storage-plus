use std::borrow::Cow;

/// The namespace of a storage container. Meant to be constructed from "stringy" types.
///
/// This type is generally not meant to be constructed directly. It's exported for
/// documentation purposes. Most of the time, you should just pass a [`String`] or
/// `&'static str` to an [`Item`](crate::Item)/collection constructor.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ns(Cow<'static, [u8]>);

impl Ns {
    pub const fn from_static_str(s: &'static str) -> Ns {
        Ns(Cow::Borrowed(s.as_bytes()))
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl From<&'static str> for Ns {
    fn from(s: &'static str) -> Self {
        Ns(Cow::Borrowed(s.as_bytes()))
    }
}

impl From<String> for Ns {
    fn from(s: String) -> Self {
        Ns(Cow::Owned(s.into_bytes()))
    }
}

impl From<Cow<'static, [u8]>> for Ns {
    fn from(s: Cow<'static, [u8]>) -> Self {
        Ns(s)
    }
}
