use vesta_macro::derive_match;
pub use vesta_macro::Match;

use std::{
    borrow::Cow,
    env::VarError,
    ffi::OsString,
    io::{ErrorKind, SeekFrom},
};

/// This module is exported so that the `derive_match!` macro can make reference to `vesta` itself
/// from within the crate.
#[doc(hidden)]
pub mod internal {
    pub use super::*;
}

pub trait Match: Sized {
    /// The tag of this value.
    ///
    /// This must return `Some(n)` exactly when `self.case<N>()` would return `Ok`, and must return
    /// `None` otherwise, including if `self.case<N>()` would be ill-typed due to a lack of a
    /// corresponding instance of [`Case`].
    fn tag(&self) -> Option<usize>;
}

pub trait Case<const N: usize>: Match {
    /// The type of the data contained in the `N`th case of the matched type.
    type Case;

    /// If the value's discriminant is `N`, return that case. Otherwise, return `self`.
    ///
    /// This must return `Ok` when `self.tag()` would return `n`, and must return `Err` otherwise.
    fn case(self) -> Result<Self::Case, Self>;

    /// Inject this case back into the matched type.
    ///
    /// Like [`Into`], this must not fail.
    fn uncase(case: Self::Case) -> Self;
}

// Implementations on foreign types:

derive_match! {
    enum Option<T> {
        None,
        Some(T),
    }
}

derive_match! {
    enum Result<T, E> {
        Ok(T),
        Err(E),
    }
}

derive_match! {
    enum Cow<'a, B> where B: 'a + ToOwned + ?Sized {
        Borrowed(&'a B),
        Owned(<B as ToOwned>::Owned),
    }
}

derive_match! {
    pub enum VarError {
        NotPresent,
        NotUnicode(OsString),
    }
}

derive_match! {
    pub enum SeekFrom {
        Start(u64),
        End(i64),
        Current(i64),
    }
}

derive_match! {
    pub enum ErrorKind {
        NotFound,
        PermissionDenied,
        ConnectionRefused,
        ConnectionReset,
        ConnectionAborted,
        NotConnected,
        AddrInUse,
        AddrNotAvailable,
        BrokenPipe,
        AlreadyExists,
        WouldBlock,
        InvalidInput,
        InvalidData,
        TimedOut,
        WriteZero,
        Interrupted,
        Other,
        UnexpectedEof,
    }
}
