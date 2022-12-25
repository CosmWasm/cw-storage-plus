// no_std support

#[cfg(feature = "std")]
pub use std::vec::Vec;

#[cfg(not(feature = "std"))]
pub use alloc::vec;

#[cfg(not(feature = "std"))]
pub use core::mem;
#[cfg(feature = "std")]
use std::mem;

pub mod ops {
    #[cfg(not(feature = "std"))]
    pub use core::ops::Deref;
    #[cfg(feature = "std")]
    use std::ops::Deref;
}

pub mod string {
    #[cfg(feature = "std")]
    pub use std::string::String;

    #[cfg(not(feature = "std"))]
    pub use alloc::string::String;
}

pub mod marker {
    #[cfg(not(feature = "std"))]
    pub use core::marker::PhantomData;
    #[cfg(feature = "std")]
    pub use std::marker::PhantomData;
}

pub mod array {
    #[cfg(feature = "std")]
    pub use std::array::TryFromSliceError;

    #[cfg(not(feature = "std"))]
    pub use core::array::TryFromSliceError;
}

pub mod convert {
    #[cfg(feature = "std")]
    pub use std::convert::TryInto;

    #[cfg(not(feature = "std"))]
    pub use core::convert::TryInto;
}

pub mod any {
    #[cfg(feature = "std")]
    pub use std::any::type_name;

    #[cfg(not(feature = "std"))]
    pub use core::any::type_name;
}

pub mod prelude {
    pub use crate::cw_std::{string::String, vec::Vec};
}
