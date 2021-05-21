/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

//! Additions to the [`Ref`](Ref) mechanism.

use std::{
    cell::Ref,
    cmp::Ordering,
    fmt::{self, Display},
    hash::{Hash, Hasher},
    ops::Deref,
};

/// A [`Ref`](Ref) that might not actually be borrowed.
/// Either a `Ptr` (a normal & style reference), or a `Ref` (like from
/// [`RefCell`](std::cell::RefCell)), but exposes all the methods available on [`Ref`](Ref).
#[derive(Debug)]
pub struct ARef<'a, T: ?Sized + 'a>(ARefInner<'a, T>);

#[derive(Debug)]
pub enum ARefInner<'a, T: ?Sized + 'a> {
    Ptr(&'a T),
    Ref(Ref<'a, T>),
}

impl<T: ?Sized> Deref for ARef<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        match &self.0 {
            ARefInner::Ptr(p) => p,
            ARefInner::Ref(p) => p.deref(),
        }
    }
}

impl<'a, T: ?Sized + 'a> ARef<'a, T> {
    /// Create a new [`ARef`] from a pointer.
    pub fn new_ptr(x: &'a T) -> Self {
        Self(ARefInner::Ptr(x))
    }

    /// Create a new [`ARef`] from a reference.
    pub fn new_ref(x: Ref<'a, T>) -> Self {
        Self(ARefInner::Ref(x))
    }

    /// See [`Ref.clone`](Ref::clone). Not a self method since that interferes with the [`Deref`](Deref).
    #[allow(clippy::should_implement_trait)]
    pub fn clone(orig: &Self) -> Self {
        match &orig.0 {
            ARefInner::Ptr(p) => Self::new_ptr(p),
            ARefInner::Ref(p) => Self::new_ref(Ref::clone(p)),
        }
    }

    /// See [`Ref.map`](Ref::map). Not a self method since that interferes with the [`Deref`](Deref).
    pub fn map<U: ?Sized, F>(orig: ARef<'a, T>, f: F) -> ARef<'a, U>
    where
        F: FnOnce(&T) -> &U,
    {
        match orig.0 {
            ARefInner::Ptr(p) => ARef::new_ptr(f(p)),
            ARefInner::Ref(p) => ARef::new_ref(Ref::map(p, f)),
        }
    }

    /// See [`Ref.map_split`](Ref::map_split). Not a self method since that interferes with the
    /// [`Deref`](Deref).
    pub fn map_split<U: ?Sized, V: ?Sized, F>(orig: ARef<'a, T>, f: F) -> (ARef<'a, U>, ARef<'a, V>)
    where
        F: FnOnce(&T) -> (&U, &V),
    {
        match orig.0 {
            ARefInner::Ptr(p) => {
                let (a, b) = f(p);
                (ARef::new_ptr(a), ARef::new_ptr(b))
            }
            ARefInner::Ref(p) => {
                let (a, b) = Ref::map_split(p, f);
                (ARef::new_ref(a), ARef::new_ref(b))
            }
        }
    }
}

// `Ref` doesn't have many traits on it. I don't really know why - I think that's an oversight.
// & references do have many traits on them. Therefore, when being "either" we choose to do as many
// implementations as we can.

impl<T: Display + ?Sized> Display for ARef<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ARef::deref(self).fmt(f)
    }
}

impl<T: Hash + ?Sized> Hash for ARef<'_, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        ARef::deref(self).hash(state)
    }
}

impl<A: PartialEq<B> + ?Sized, B: ?Sized> PartialEq<ARef<'_, B>> for ARef<'_, A> {
    fn eq(&self, other: &ARef<'_, B>) -> bool {
        ARef::deref(self).eq(ARef::deref(other))
    }
}

impl<A: Eq + ?Sized> Eq for ARef<'_, A> {}

impl<A: PartialOrd<B> + ?Sized, B: ?Sized> PartialOrd<ARef<'_, B>> for ARef<'_, A> {
    fn partial_cmp(&self, other: &ARef<'_, B>) -> Option<Ordering> {
        ARef::deref(self).partial_cmp(ARef::deref(other))
    }
}

impl<A: Ord + ?Sized> Ord for ARef<'_, A> {
    fn cmp(&self, other: &Self) -> Ordering {
        ARef::deref(self).cmp(ARef::deref(other))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{cell::RefCell, mem};

    #[test]
    fn test_from_ref_docs() {
        let c = RefCell::new((5, 'b'));
        let b1: ARef<(u32, char)> = ARef::new_ref(c.borrow());
        let b2: ARef<u32> = ARef::map(b1, |t| &t.0);
        assert_eq!(*b2, 5);

        let cell = RefCell::new([1, 2, 3, 4]);
        let borrow = ARef::new_ref(cell.borrow());
        let (begin, end) = ARef::map_split(borrow, |slice| slice.split_at(2));
        assert_eq!(*begin, [1, 2]);
        assert_eq!(*end, [3, 4]);
    }

    #[test]
    fn test_borrow_guards() {
        let c = RefCell::new(5);
        assert!(c.try_borrow_mut().is_ok());
        let r1 = ARef::new_ref(c.borrow());
        assert!(c.try_borrow_mut().is_err());
        let r2 = c.borrow();
        assert!(c.try_borrow_mut().is_err());
        mem::drop(r1);
        assert!(c.try_borrow_mut().is_err());
        mem::drop(r2);
        assert!(c.try_borrow_mut().is_ok());
    }

    #[test]
    fn test_pointer_basics() {
        let c = "test".to_owned();
        let p = ARef::new_ptr(&c);
        let p2 = ARef::map(p, |x| &x[1..3]);
        assert_eq!(&*p2, "es");
    }

    #[test]
    fn test_ref_map_dropping() {
        let c = RefCell::new("test".to_owned());
        let p = ARef::new_ref(c.borrow());
        let p = ARef::map(p, |x| &x[1..3]);
        assert_eq!(&*p, "es");
        mem::drop(p);
        assert!(c.try_borrow_mut().is_ok());
    }
}
