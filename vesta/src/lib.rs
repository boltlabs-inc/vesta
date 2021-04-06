use vesta_macro::derive_match;
pub use vesta_macro::{case, Match};

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

/// A type which is [`Match`] can be pattern-matched using Vesta's extensible pattern matching.
///
/// In order for a type to be matched, it must implement [`Match`], as well as [`Case`] for each
/// distinct case it can be matched against.
pub unsafe trait Match: Sized {
    /// The tag of this value.
    ///
    /// # Safety
    ///
    /// It is undefined behavior for this function to return `Some(n)` if `<Self as
    /// Case<N>>::case(self)` would be undefined behavior. In other words: returning `Some(n)` is a
    /// *guarantee* that it is safe to call [`case`](Case::case) for this value at the type level
    /// tag `N`.
    ///
    /// If an instance of [`Exhaustive<M>`] exists for this type, whenever [`tag`](Match::tag)
    /// returns `Some(n)`, `n` must be *strictly less than* `M`. Undefined behavior may result if
    /// this guarantee is violated.
    ///
    /// This function should always return the same result, no matter when it is called on `self`,
    /// and no matter how many times. In general, it is impossible to safely implement [`Match`] for
    /// types with interior mutability, unless that interior mutability has no ability to change the
    /// tag. When pattern-matching occurs, there is no guarantee that `self.tag()` is checked and
    /// `self.case()` subsequently called (if applicable) in a single atomic action.
    ///
    /// It is always safe to return `None` from this function, but that indicates that this value
    /// cannot be pattern-matched.
    fn tag(&self) -> Option<usize>;
}

/// A type which is [`Exhaustive<N>`](Exhaustive) guarantees that an `N`-ary match statement is
/// exhaustive.
///
/// # Safety
///
/// An instance of [`Exhaustive<N>`](Exhaustive) guarantees there is a [`Case<M>`](Case)
/// implementation for every `M` *strictly less than* `N` and *never* a [`Case<M>`](Case)
/// implementation for any `M` equal to or greater than `N`. Undefined behavior may result if there
/// exists a [`Case`] implementation violating this guarantee.
///
/// There must be *at most one* instance of [`Exhaustive`] for any given type. Undefined behavior
/// may result if `T: Exhaustive<N> + Exhaustive<M>` where `N != M`.
///
/// This trait implicitly serves as a guarantee on the output range of [`tag`](Match::tag) for the
/// type. It is undefined behavior if [`tag`](Match::tag) ever returns a value greater than or equal
/// to `N`.
pub unsafe trait Exhaustive<const N: usize>: Match {}

pub trait AssertExhaustive<const N: usize>: Match {
    /// Statically assert that the type is exhaustive for `N`.
    ///
    /// This function can only be called if `Self: Exhaustive<N>`. It does nothing when called.
    #[inline(always)]
    fn assert_exhaustive(&self)
    where
        Self: Exhaustive<N>,
    {
    }
}

impl<T: Match, const N: usize> AssertExhaustive<N> for T {}

/// An implementation of [`Case`] defines a particular case of a pattern match for a type.
pub trait Case<const N: usize>: Match {
    /// The type of the data contained in the `N`th case of the matched type.
    type Case;

    /// If the value's discriminant is `N`, return that case.
    ///
    /// # Safety
    ///
    /// It is undefined behavior to call this function when `self.tag()` would return anything other
    /// than `Some(n)`, where `n = N`.
    unsafe fn case(self) -> Self::Case;

    /// If the value's discriminant is `N`, return that case; otherwise, return `self`.
    ///
    /// In its default implementation, this method checks that `self.tag() == N` and then calls
    /// `self.case()` only if so. In the case where this method can be more efficiently implemented
    /// than the composition of `self.tag()` with `self.case()`, this method can be overloaded.
    fn try_case(self) -> Result<Self::Case, Self> {
        if self.tag() == Some(N) {
            // It is safe to call `self.case()` because we have checked the tag
            Ok(unsafe { self.case() })
        } else {
            Err(self)
        }
    }

    /// Inject this case back into the matched type.
    ///
    /// This operation must not panic or otherwise fail.
    fn uncase(case: Self::Case) -> Self;
}

fn foo() {
    case!(None::<bool> {
        0(_) => todo!(),
        0(()) => todo!(),
        _ => todo!(),
        1(true) => todo!(),
        1 => todo!(),
        // _ => todo!(),
    })
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
    #[non_exhaustive]
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
