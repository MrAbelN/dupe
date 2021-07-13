/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

//! A trait to represent zero-cost conversions.

// TODO(ndmitchell): We could derive instances, similarly to `ref-cast`.
// Leave that as future work if it turns out to be a useful idea.

use crate::cast::{self, transmute_unchecked};
use std::collections::{HashMap, HashSet};

/// A marker trait such that the existence of `From: Coerce<To>` implies
/// that `From` can be treat as `To` without any data manipulation.
/// Particularly useful for containers, e.g. `Vec<From>` can be treated as
/// `Vec<To>` in _O(1)_. If such an instance is available,
/// you can use [`coerce`] and [`coerce_ref`] to perform the conversion.
///
/// Importantly, you must make sure Rust does not change the type representation
/// between the different types (typically using a `repr` directive),
/// and it must be safe for the `From` to be treated as `To`, namely same (or less restrictive) alignment,
/// no additional invariants, value can be dropped as `To`.
///
/// One use of `Coerce` is around newtype wrappers:
///
/// ```
/// use gazebo::coerce::{Coerce, coerce, coerce_ref};
/// #[repr(transparent)]
/// #[derive(Debug)]
/// struct Wrapper(String);
/// unsafe impl Coerce<String> for Wrapper {}
///
/// let value = vec![Wrapper("hello".to_owned()), Wrapper("world".to_owned())];
/// assert_eq!(
///     coerce_ref::<_, Vec<String>>(&value).join(" "),
///     "hello world"
/// );
/// let mut value = coerce::<_, Vec<String>>(value);
/// assert_eq!(value.pop(), Some("world".to_owned()));
/// ```
///
/// Another involves containers:
///
/// ```
/// use gazebo::coerce::{Coerce, coerce_ref};
/// # #[repr(transparent)]
/// # struct Wrapper(String);
/// # unsafe impl Coerce<String> for Wrapper {}
/// #[repr(C)]
/// struct Container<T>(i32, T);
/// unsafe impl<From, To> Coerce<Container<To>> for Container<From> where From: Coerce<To> {}
///
/// let value = Container(20, Wrapper("twenty".to_owned()));
/// assert_eq!(
///     coerce_ref::<_, Container<String>>(&value).1,
///     "twenty"
/// );
/// ```
///
/// If you only need [`coerce_ref`] on newtypes, then the [`ref-cast` crate](https://crates.io/crates/ref-cast)
/// provides that, along with automatic derivations (no `unsafe` required).
pub unsafe trait Coerce<To> {}

/// A marker trait such that the existence of `From: CoerceKey<To>` implies
/// that `From` can be treat as `To` without any data manipulation.
/// Furthermore, above and beyond [`Coerce`], any provided [`Hash`](std::hash::Hash),
/// [`Eq`], [`PartialEq`], [`Ord`] and [`PartialOrd`] traits must give identical results
/// on the `From` and `To` values.
///
/// This trait is mostly expected to be a requirement for the keys of associative-map
/// containers, hence the `Key` in the name.
pub unsafe trait CoerceKey<To>: Coerce<To> {}

unsafe impl<From, To> Coerce<Vec<To>> for Vec<From> where From: Coerce<To> {}
unsafe impl<From, To> CoerceKey<Vec<To>> for Vec<From> where From: CoerceKey<To> {}

unsafe impl<From, To> CoerceKey<Box<To>> for Box<From> where From: CoerceKey<To> {}
unsafe impl<From, To> Coerce<Box<To>> for Box<From> where From: Coerce<To> {}

unsafe impl<From, To> Coerce<HashSet<To>> for HashSet<From> where From: CoerceKey<To> {}

unsafe impl<FromK, FromV, ToK, ToV> Coerce<HashMap<ToK, ToV>> for HashMap<FromK, FromV>
where
    FromK: CoerceKey<ToK>,
    FromV: Coerce<ToV>,
{
}

unsafe impl<From1, From2, To1, To2> Coerce<(To1, To2)> for (From1, From2)
where
    From1: Coerce<To1>,
    From2: Coerce<To2>,
{
}

unsafe impl<From1, From2, To1, To2> CoerceKey<(To1, To2)> for (From1, From2)
where
    From1: CoerceKey<To1>,
    From2: CoerceKey<To2>,
{
}

// We can't define a blanket `Coerce<T> for T` because that conflicts with the specific traits above.
// Therefore, we define instances where we think they might be useful, rather than trying to do every concrete type.
unsafe impl Coerce<String> for String {}
unsafe impl CoerceKey<String> for String {}

/// Safely convert between types which have a `Coerce` relationship.
/// Often the second type argument will need to be given explicitly,
/// e.g. `coerce::<_, ToType>(x)`.
pub fn coerce<From, To>(x: From) -> To
where
    From: Coerce<To>,
{
    unsafe { transmute_unchecked(x) }
}

/// Safely convert between types which have a `Coerce` relationship.
/// Often the second type argument will need to be given explicitly,
/// e.g. `coerce_ref::<_, ToType>(x)`.
pub fn coerce_ref<From, To>(x: &From) -> &To
where
    From: Coerce<To>,
{
    unsafe { cast::ptr(x) }
}
