use std::borrow::Cow;

/// Types that can be used as top-level storage keys for storage items and collections.
pub trait Namespace {
    fn namespace(self) -> Ns;
}

impl Namespace for Ns {
    fn namespace(self) -> Ns {
        self
    }
}

impl Namespace for &'static str {
    fn namespace(self) -> Ns {
        Ns::from_static_str(self)
    }
}

impl Namespace for String {
    fn namespace(self) -> Ns {
        Ns(Cow::Owned(self.into_bytes()))
    }
}

/// The namespace of a storage container.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ns(Cow<'static, [u8]>);

impl Ns {
    pub(crate) const fn from_static_str(s: &'static str) -> Ns {
        Ns(Cow::Borrowed(s.as_bytes()))
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_ref()
    }
}
