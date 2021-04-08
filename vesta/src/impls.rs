use std::{
    borrow::Cow,
    convert::Infallible,
    env::VarError,
    ffi::{OsStr, OsString},
    fmt::Alignment,
    io::{ErrorKind, SeekFrom},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, SocketAddrV4, SocketAddrV6},
    num::FpCategory,
    ops::Bound,
    path::{Component, Prefix, PrefixComponent},
    sync::{
        mpsc::{RecvTimeoutError, TryRecvError, TrySendError},
        PoisonError, TryLockError,
    },
};
use vesta_macro::derive_match;

derive_match! {
    pub enum Infallible {}
}

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
    pub enum Bound<T> {
        Included(T),
        Excluded(T),
        Unbounded,
    }
}

derive_match! {
    pub enum IpAddr {
        V4(Ipv4Addr),
        V6(Ipv6Addr),
    }
}

derive_match! {
    pub enum SocketAddr {
        V4(SocketAddrV4),
        V6(SocketAddrV6),
    }
}

derive_match! {
    pub enum Shutdown {
        Read,
        Write,
        Both,
    }
}

derive_match! {
    pub enum TryLockError<T> {
        Poisoned(PoisonError<T>),
        WouldBlock,
    }
}

derive_match! {
    pub enum TryRecvError {
        Empty,
        Disconnected,
    }
}

derive_match! {
    pub enum RecvTimeoutError {
        Timeout,
        Disconnected,
    }
}

derive_match! {
    pub enum TrySendError<T> {
        Full(T),
        Disconnected(T),
    }
}

derive_match! {
    pub enum FpCategory {
        Nan,
        Infinite,
        Zero,
        Subnormal,
        Normal,
    }
}

derive_match! {
    pub enum Alignment {
        Left,
        Right,
        Center,
    }
}

derive_match! {
    pub enum Prefix<'a> {
        Verbatim(&'a OsStr),
        VerbatimUNC(&'a OsStr, &'a OsStr),
        VerbatimDisk(u8),
        DeviceNS(&'a OsStr),
        UNC(&'a OsStr, &'a OsStr),
        Disk(u8),
    }
}

derive_match! {
    pub enum Component<'a> {
        Prefix(PrefixComponent<'a>),
        RootDir,
        CurDir,
        ParentDir,
        Normal(&'a OsStr),
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

mod cmp {
    use super::*;
    use std::cmp::Ordering;

    derive_match! {
        pub enum Ordering {
            Less,
            Equal,
            Greater,
        }
    }
}

mod atomic {
    use super::*;
    use std::sync::atomic::Ordering;

    derive_match! {
        #[non_exhaustive]
        pub enum Ordering {
            Relaxed,
            Release,
            Acquire,
            AcqRel,
            SeqCst,
        }
    }
}

mod btree_map {
    use super::*;
    use std::collections::btree_map::*;

    derive_match! {
        pub enum Entry<'a, K, V>
        where
            K: 'a,
            V: 'a,
        {
            Vacant(VacantEntry<'a, K, V>),
            Occupied(OccupiedEntry<'a, K, V>),
        }
    }
}

mod hash_map {
    use super::*;
    use std::collections::hash_map::*;

    derive_match! {
        pub enum Entry<'a, K, V>
        where
            K: 'a,
            V: 'a,
        {
            Vacant(VacantEntry<'a, K, V>),
            Occupied(OccupiedEntry<'a, K, V>),
        }
    }
}
