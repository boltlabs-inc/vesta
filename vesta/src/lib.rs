//! [![Rust](https://github.com/boltlabs-inc/vesta/actions/workflows/rust.yml/badge.svg)](https://github.com/boltlabs-inc/vesta/actions/workflows/rust.yml)
//! ![license: MIT](https://img.shields.io/github/license/boltlabs-inc/vesta)
//! [![crates.io](https://img.shields.io/crates/v/vesta)](https://crates.io/crates/vesta)
//! [![docs.rs documentation](https://docs.rs/vesta/badge.svg)](https://docs.rs/vesta)
//!
//! > A **_vesta_**, otherwise known as a *match case*, is a small container for matches, named
//! > after the Roman goddess of the hearth.
//! >
//! > **Vesta** is a crate for extensibly *matching cases* in Rust.
//!
//! By implementing [`Match`](Match@trait) and [`Case`] for some type (or better yet, correctly
//! deriving them using the [`Match`](Match@macro) derive macro), you can pattern-match on that type
//! using the [`case!`] macro almost like using the `match` keyword built into Rust.
//!
//! However, Vesta's [`case!`] macro is more general than `match`, because [`Match`] and [`Case`]
//! are traits! This means you can enable pattern-matching for types which are not literally
//! implemented as `enum`s, and you can write code which is generic over any type that is
//! pattern-matchable.

#![warn(missing_docs)]
#![warn(missing_copy_implementations, missing_debug_implementations)]
#![warn(unused_qualifications, unused_results)]
#![warn(future_incompatible)]
#![warn(unused)]
// Documentation configuration
#![forbid(broken_intra_doc_links)]

pub use vesta_macro::{case, Match};

/// This module is exported so that the `derive_match!` macro can make reference to `vesta` itself
/// from within the crate.
#[doc(hidden)]
pub mod vesta {
    pub use super::*;
}

/// A type which is [`Match`] can be pattern-matched using the [`case!`] macro and the methods of
/// [`CaseExt`]/[`Case`].
///
/// In order for a type to be matched, it must implement [`Match`], as well as [`Case`] for each
/// distinct case it can be matched against.
pub unsafe trait Match: Sized {
    /// The range of [`tag`](Match::tag) for this type: either [`Nonexhaustive`], or
    /// [`Exhaustive<N>`](Exhaustive) for some `N`.
    ///
    /// No other types are permissible for this associated type; it is constrained by the sealed
    /// `Range` trait, which is only implemented for these two options.
    ///
    /// # Safety
    ///
    /// If the [`Range`](Match::Range) is [`Exhaustive<N>`](Exhaustive), then [`tag`](Match::tag)
    /// must *never* return `None`. For all `Some(m)` it returns, `m` must be *strictly less than*
    /// `N`. Undefined behavior may result if this guarantee is violated.
    type Range: sealed::Range;

    /// The tag of this value.
    ///
    /// # Safety
    ///
    /// If this function returns `Some(n)`, this is a *guarantee* that it is safe to call
    /// [`case`](Case::case) for this value at the type level tag `N = n`. It is undefined behavior
    /// for this function to return `Some(n)` if `<Self as Case<N>>::case(self)` would be unsafe.
    ///
    /// If the [`Range`](Match::Range) is [`Exhaustive<N>`](Exhaustive), then this function must
    /// *never* return `None`. For all `Some(m)` it returns, `m` must be *strictly less than* `N`.
    /// Undefined behavior may result if this guarantee is violated.
    ///
    /// Only if the [`Range`](Match::Range) is [`Nonexhaustive`] is it safe for this function to
    /// return `None`. Returning `None` will cause all pattern matches on this value to take the
    /// default case.
    ///
    /// This function should always return the same result. In general, it is impossible to safely
    /// implement [`Match`] for types with interior mutability, unless that interior mutability has
    /// no ability to change the tag. When pattern-matching occurs, there is no guarantee that
    /// `self.tag()` is checked and `self.case()` subsequently called (if applicable) in a single
    /// atomic action, which may lead to undefined behavior if the tag changes between these two
    /// moments.
    ///
    /// # Examples
    ///
    /// ```
    /// use vesta::Match;
    ///
    /// assert_eq!(Some(0), None::<bool>.tag());
    /// assert_eq!(Some(1), Some(true).tag());
    /// ```
    fn tag(&self) -> Option<usize>;
}

/// An extension trait providing methods analogous to those in [`Case`], but which take `self` and
/// type parameters.<br>💡 Prefer using these to directly calling the methods in [`Case`].
pub trait CaseExt: Sized {
    /// If the value's [`tag`](Match::tag) is `N`, return that case.
    ///
    /// # Safety
    ///
    /// It is undefined behavior to call this function when [`self.tag()`](Match::tag) would return
    /// anything other than `Some(n)`, where `n = N`.
    ///
    /// # Examples
    ///
    /// ```
    /// use vesta::{Match, CaseExt};
    ///
    /// let option = Some("hello");
    /// assert_eq!(option.tag(), Some(1));
    /// let string = unsafe { option.case::<1>() };
    /// assert_eq!(string, "hello");
    /// ```
    #[inline(always)]
    unsafe fn case<const N: usize>(self) -> Self::Case
    where
        Self: Case<N>,
    {
        Case::case(self)
    }

    /// If the value's [`tag`](Match::tag) is `N`, return that case; otherwise, return `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use vesta::CaseExt;
    ///
    /// let result = Some("hello").try_case::<1>();
    /// assert_eq!(result, Ok("hello"));
    /// ```
    #[inline(always)]
    fn try_case<const N: usize>(self) -> Result<Self::Case, Self>
    where
        Self: Case<N>,
    {
        Case::try_case(self)
    }

    /// The inverse of [`case`](CaseExt::case): inject this case back into the matched type.
    ///
    /// This operation must not panic or otherwise fail.
    ///
    /// # Examples
    ///
    /// ```
    /// use vesta::CaseExt;
    ///
    /// let option: Option<_> = "hello".uncase::<_, 1>();
    /// assert_eq!(option, Some("hello"));
    /// ```
    #[inline(always)]
    fn uncase<T, const N: usize>(self) -> T
    where
        T: Case<N, Case = Self>,
    {
        Case::uncase(self)
    }
}

impl<T: Sized> CaseExt for T {}

/// Statically assert that the type of the given value is exhaustive for `N`.
///
/// This function can only be called if `Self: Match<Range = Exhaustive<N>>`. It does nothing
/// when called.
///
/// # Examples
///
/// ```
/// vesta::assert_exhaustive::<_, 2>(&Some(true));
/// ```
#[inline(always)]
pub fn assert_exhaustive<T, const N: usize>(_: &T)
where
    T: Match<Range = Exhaustive<N>>,
{
}

/// Mark an unreachable location in generated code.
///
/// # Panics
///
/// In debug mode, panics immediately when this function is called.
///
/// # Safety
///
/// In release mode, undefined behavior may occur if this function is ever called.
#[doc(hidden)]
#[inline(always)]
pub unsafe fn unreachable<T>() -> T {
    #[cfg(release)]
    {
        core::hint::unreachable_unchecked()
    }
    #[cfg(not(release))]
    {
        core::unreachable!("invariant violation in `vesta::Match` or `vesta::Case` implementation")
    }
}

/// A marker type indicating that the [`tag`](Match::tag) for some type will always be *strictly
/// less than* `N`.
///
/// Use this to mark the [`Range`](Match::Range) of exhaustive enumerations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Exhaustive<const N: usize> {}

/// A marker type indicating that the [`tag`](Match::tag) for some type is not fixed to some known
/// upper bound.
///
/// Use this to mark the [`Range`](Match::Range) of non-exhaustive enumerations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Nonexhaustive {}

/// An implementation of [`Case`] defines a particular case of a pattern match for a type.<br> ℹ️
/// Prefer using the methods of [`CaseExt`] to directly calling these methods.
pub trait Case<const N: usize>: Match {
    /// The type of the data contained in the `N`th case of the matched type.
    type Case;

    /// If the value's [`tag`](Match::tag) is `N`, return that case.
    ///
    /// # Safety
    ///
    /// It is undefined behavior to call this function when [`self.tag()`](Match::tag) would return
    /// anything other than `Some(n)`, where `n = N`.
    ///
    /// # Examples
    ///
    /// ```
    /// use vesta::{Match, Case};
    ///
    /// let option = Some("hello");
    /// assert_eq!(option.tag(), Some(1));
    /// let string = unsafe { <_ as Case<1>>::case(option) };
    /// assert_eq!(string, "hello");
    /// ```
    unsafe fn case(this: Self) -> Self::Case;

    /// If the value's [`tag`](Match::tag) is `N`, return that case; otherwise, return `self`.
    ///
    /// In its default implementation, this method checks that `self.tag() == N` and then calls
    /// [`case`](Case::case) only if so.
    ///
    /// In the case where this method can be more efficiently implemented than the composition of
    /// [`tag`](Match::tag) with [`case`](Case::case), this method can be overloaded.
    ///
    /// # Examples
    ///
    /// ```
    /// use vesta::Case;
    ///
    /// let result = <_ as Case<1>>::try_case(Some("hello"));
    /// assert_eq!(result, Ok("hello"));
    /// ```
    fn try_case(this: Self) -> Result<Self::Case, Self> {
        if this.tag() == Some(N) {
            // It is safe to call `self.case()` because we have checked the tag
            Ok(unsafe { Case::case(this) })
        } else {
            Err(this)
        }
    }

    /// The inverse of [`case`](Case::case): inject this case back into the matched type.
    ///
    /// This operation must not panic or otherwise fail.
    ///
    /// # Examples
    ///
    /// ```
    /// use vesta::Case;
    ///
    /// let option: Option<_> = <_ as Case<1>>::uncase("hello");
    /// assert_eq!(option, Some("hello"));
    /// ```
    fn uncase(case: Self::Case) -> Self;
}

mod sealed {
    pub trait Range {}
    impl<const N: usize> Range for super::Exhaustive<N> {}
    impl Range for super::Nonexhaustive {}
}

mod impls;
