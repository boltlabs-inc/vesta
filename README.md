# Vesta

[![Rust](https://github.com/boltlabs-inc/vesta/actions/workflows/rust.yml/badge.svg)](https://github.com/boltlabs-inc/vesta/actions/workflows/rust.yml)
![license: MIT](https://img.shields.io/github/license/boltlabs-inc/vesta)
[![crates.io](https://img.shields.io/crates/v/vesta)](https://crates.io/crates/vesta)
[![docs.rs documentation](https://docs.rs/vesta/badge.svg)](https://docs.rs/vesta)

> A **vesta**, otherwise known as a *match case*, is a small container for matches, named
> after the Roman goddess of the hearth.
>
> **Vesta** is a crate for extensibly *matching cases* in Rust.

By implementing `Match` and `Case` for some type (or better yet, correctly deriving them using the
`Match` derive macro), you can pattern-match on that type using the `case!` macro almost like using
the `match` keyword built into Rust.

However, Vesta's `case!` macro is more general than `match`, because `Match` and `Case` are traits!
This means you can enable pattern-matching for types which are not literally implemented as `enum`s,
and you can write code which is generic over any type that is pattern-matchable.
