use std::{borrow::Cow, ops::Deref};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ns(Cow<'static, [u8]>);

impl Ns {
    pub const fn from_static_str(s: &'static str) -> Ns {
        Ns(Cow::Borrowed(s.as_bytes()))
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

impl Deref for Ns {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}
