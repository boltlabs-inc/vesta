# Vesta

> A **_vesta_**, otherwise known as a *match case*, is a small container for matches, named
> after the Roman goddess of the hearth.
>
> **Vesta** is a crate for extensibly *matching cases* in Rust.

By implementing `Match` and `Case` for some type (or better yet, correctly deriving them using the
`Match` derive macro), you can pattern-match on that type using the `case!` macro almost like using
the `match` keyword built into Rust.

However, Vesta's `case!` macro is more general than `match`, because `Match` and `Case` are traits!
This means you can enable pattern-matching for types which are not literally implemented as `enum`s,
and you can write code which is generic over any type that is pattern-matchable.
