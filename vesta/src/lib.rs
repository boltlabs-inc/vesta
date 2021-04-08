use std::{
    borrow::Cow,
    env::VarError,
    ffi::OsString,
    io::{ErrorKind, SeekFrom},
};

use vesta_macro::derive_match;

/// Derive correct and efficient instances of [`Match`] and [`Case`] for a given `struct` or `enum`.
///
/// # Examples
///
/// ```
/// use vesta::{Match, case};
///
/// #[derive(Match)]
/// enum T<'a, P> {
///     A,
///     B(i64),
///     C { field: P },
///     D(&'a str, bool),
/// }
///
/// fn check<'a>(t: T<'a, usize>) -> bool {
///     case!(t {
///         0 => true,
///         1(0) => true,
///         1(n) => n != 0,
///         2(u) if u == 6 => u % 2 == 0,
///         2 => true,
///         3(s, true) => s.chars().count() % 2 == 0,
///         3(s, _) => true,
///     })
/// }
///
/// use T::*;
///
/// assert!(check(A));
/// assert!(check(B(0)));
/// assert!(check(B(1)));
/// assert!(check(C { field: 0 }));
/// assert!(check(C { field: 6 }));
/// assert!(check(D("hello", false)));
/// assert!(check(D("world!", true)));
/// ```
pub use vesta_macro::Match;

/// Match on the [`Case`](Case::Case)s of a value implementing [`Match`].
///
/// This macro is the safe and efficient way to match on something; it is faster than using chains
/// of [`try_case`](Case::try_case), but safe because it ensures exhaustiveness when required.
///
/// # Examples
///
/// ```
/// use vesta::case;
///
/// let option: Option<&str> = Some("thing");
///
/// case!(option {
///     0 => assert!(false),
///     1(s) => assert_eq!(s, "thing"),
/// });
/// ```
pub use vesta_macro::case;

/// This module is exported so that the `derive_match!` macro can make reference to `vesta` itself
/// from within the crate.
#[doc(hidden)]
pub mod vesta {
    pub use super::*;
}

/// A type which is [`Match`] can be pattern-matched using Vesta's extensible pattern matching.
///
/// In order for a type to be matched, it must implement [`Match`], as well as [`Case`] for each
/// distinct case it can be matched against.
pub unsafe trait Match: Sized {
    /// The range of [`tag`](Match::tag) for this type: either [`Nonexhaustive`], or
    /// [`Exhaustive<N>`](Exhaustive) for some `N`.
    ///
    /// # Safety
    ///
    /// If the [`Range`](Match::Range) is [`Exhaustive<N>`](Exhaustive), then [`tag`](Match::tag) must
    /// *never* return `None`. For all `Some(m)` it returns, `m` must be *strictly less than* `N`.
    /// Undefined behavior may result if this guarantee is violated.
    type Range: sealed::Range;

    /// The tag of this value.
    ///
    /// # Safety
    ///
    /// If this function returns `Some(n)`, this is a *guarantee* that it is safe to call
    /// [`case`](Case::case) for this value at the type level tag `N = n`. It is undefined behavior
    /// for this function to return `Some(n)` if `<Self as Case<N>>::case(self)` would be unsafe.
    ///
    /// If the [`Range`](Match::Range) is [`Exhaustive<N>`](Exhaustive), then this function must *never*
    /// return `None`. For all `Some(m)` it returns, `m` must be *strictly less than* `N`. Undefined
    /// behavior may result if this guarantee is violated.
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

/// Statically assert that the type is exhaustive for `N`.
///
/// This function can only be called if `Self: Match<Range = Exhaustive<N>>`. It does nothing
/// when called.
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
pub unsafe fn unreachable<T>() -> T {
    #[cfg(release)]
    {
        std::hint::unreachable_unchecked()
    }
    #[cfg(not(release))]
    {
        unreachable!("invariant violation in `vesta::Match` or `vesta::Case` implementation")
    }
}

/// A marker type indicating that the [`tag`](Match::tag) for some type will always be *strictly
/// less than* `N`.
///
/// Use this to mark the [`Range`](Match::Range) of exhaustive enumerations.
pub enum Exhaustive<const N: usize> {}

/// A marker type indicating that the [`tag`](Match::tag) for some type is not fixed to some known
/// upper bound.
///
/// Use this to mark the [`Range`](Match::Range) of non-exhaustive enumerations.
pub enum Nonexhaustive {}

/// An implementation of [`Case`] defines a particular case of a pattern match for a type.
pub trait Case<const N: usize>: Match {
    /// The type of the data contained in the `N`th case of the matched type.
    type Case;

    /// If the value's [`tag`](Match::tag) is `N`, return that case.
    ///
    /// # Safety
    ///
    /// It is undefined behavior to call this function when [`self.tag()`](Match::tag) would return
    /// anything other than `Some(n)`, where `n = N`.
    unsafe fn case(self) -> Self::Case;

    /// If the value's [`tag`](Match::tag) is `N`, return that case; otherwise, return `self`.
    ///
    /// In its default implementation, this method checks that `self.tag() == N` and then calls
    /// [`self.case()`](Case::case) only if so.
    ///
    /// In the case where this method can be more efficiently implemented than the composition of
    /// `self.tag()` with `self.case()`, this method can be overloaded.
    fn try_case(self) -> Result<Self::Case, Self> {
        if self.tag() == Some(N) {
            // It is safe to call `self.case()` because we have checked the tag
            Ok(unsafe { self.case() })
        } else {
            Err(self)
        }
    }

    /// The inverse of [`case`](Case::case): inject this case back into the matched type.
    ///
    /// This operation must not panic or otherwise fail.
    fn uncase(case: Self::Case) -> Self;
}

mod sealed {
    pub trait Range {}
    impl<const N: usize> Range for super::Exhaustive<N> {}
    impl Range for super::Nonexhaustive {}
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
