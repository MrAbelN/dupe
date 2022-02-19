/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

//! Methods that build upon the [`Any` trait](Any).

use std::any::{type_name, Any, TypeId};

pub use gazebo_derive::AnyLifetime;

/// Like [`Any`](Any), but instead of being a value of non-statically-determined type,
/// provides a result into which a specific type can be written.
///
/// ```
/// use gazebo::any::AnyResult;
/// let mut res = AnyResult::new::<String>();
/// res.add(|| String::from("Hello"));
/// res.add(|| "goodbye");
/// res.add(|| 42);
/// assert_eq!(res.result::<String>(), Some(String::from("Hello")));
/// ```
pub struct AnyResult {
    want: TypeId,
    want_name: &'static str,
    result: Option<Box<dyn Any + Send>>,
}

impl AnyResult {
    /// Create a new [`AnyResult`](AnyResult) that can contain values of type `T`.
    pub fn new<T: 'static>() -> Self {
        Self {
            want: TypeId::of::<T>(),
            want_name: type_name::<T>(),
            result: None,
        }
    }

    /// Grab the value stored in an [`AnyResult`](AnyResult). Returns [`None`](None) if no value
    /// with the right type has been added, otherwise the first value that was
    /// added. It is an error to call `result` with a different type to that
    /// which was used with [`new`](AnyResult::new).
    pub fn result<T: 'static>(self) -> Option<T> {
        // the Err doesn't contain the error, just the left over input
        if self.want != TypeId::of::<T>() {
            panic!(
                "AnyResult new/result used at different types, new={}, result={}",
                self.want_name,
                type_name::<T>()
            )
        }
        match self.result {
            None => None,
            Some(v) => match v.downcast() {
                Ok(v) => Some(*v),
                _ => unreachable!(), // would have panic'd above
            },
        }
    }

    /// Same as [`result`](AnyResult::result), but gets a reference.
    pub fn result_ref<T: 'static>(&self) -> Option<&T> {
        // the Err doesn't contain the error, just the left over input
        if self.want != TypeId::of::<T>() {
            panic!(
                "AnyResult new/result used at different types, new={}, result={}",
                self.want_name,
                type_name::<T>()
            )
        }
        match &self.result {
            None => None,
            Some(v) => match v.downcast_ref() {
                Some(v) => Some(v),
                _ => unreachable!(), // would have panic'd above
            },
        }
    }

    /// Add a value with a given type to the [`AnyResult`](AnyResult). If this call is the first
    /// where the type `T` matches that used for [`new`](AnyResult::new) then the function will be run.
    pub fn add<T: 'static + Send, F: FnOnce() -> T>(&mut self, f: F) -> &mut Self {
        if TypeId::of::<T>() == self.want {
            // If we already have a value, the user called add twice at one type, which
            // isn't a great idea but if we only fail when the type matches
            // `want`, we sometimes get errors and sometimes don't so instead
            // specify that first-result wins
            if self.result.is_none() {
                self.result = Some(Box::new(f()));
            }
        }
        self
    }
}

/// Provides access to the same type as `Self` but with all lifetimes dropped to `'static`
/// (including lifetimes of parameters).
///
/// This type is usually implemented with `#[derive(AnyLifetime)]`.
pub unsafe trait ProvidesStaticType {
    /// Same type as `Self` but with lifetimes dropped to `'static`.
    ///
    /// The trait is unsafe because if this is implemented incorrectly,
    /// the program might not work correctly.
    type StaticType: 'static + ?Sized;
}

/// Any `ProvidesStaticType` can implement `AnyLifetime`.
///
/// Note `ProvidesStaticType` and `AnyLifetime` cannot be the same type,
/// because `AnyLifetime` need to be object safe,
/// and `ProvidesStaticType` has type member.
unsafe impl<'a, T: ProvidesStaticType + 'a + ?Sized> AnyLifetime<'a> for T {
    fn static_type_id() -> TypeId
    where
        Self: Sized,
    {
        TypeId::of::<T::StaticType>()
    }

    fn static_type_of(&self) -> TypeId {
        TypeId::of::<T::StaticType>()
    }
}

/// Like [`Any`](Any), but while [`Any`](Any) requires `'static`, this version allows a
/// lifetime parameter.
///
/// Code using this trait is _unsafe_ if your implementation of the inner
/// methods do not meet the invariants listed. Therefore, it is recommended you
/// use one of the helper macros.
///
/// If your data type is of the form `Foo` or `Foo<'v>` you can derive
/// `AnyLifetime`:
///
/// ```
/// use gazebo::any::AnyLifetime;
/// #[derive(AnyLifetime)]
/// struct Foo1();
/// #[derive(AnyLifetime)]
/// struct Foo2<'a>(&'a ());
/// ```
///
/// For more complicated context or constraints, you can implement `ProvidesStaticType`
/// directly.
///
/// ```
/// use gazebo::any::ProvidesStaticType;
/// # fn main() {
/// # use std::fmt::Display;
/// struct Baz<T: Display>(T);
/// # // TODO: `#[derive(AnyLifetime)]` should learn to handle this case too.
/// unsafe impl<T> ProvidesStaticType for Baz<T>
///     where
///         T: ProvidesStaticType + Display,
///         T::StaticType: Display + Sized,
/// {
///     type StaticType = Baz<T::StaticType>;
/// }
/// # }
/// ```
pub unsafe trait AnyLifetime<'a>: 'a {
    /// Must return the `TypeId` of `Self` but where the lifetimes are changed
    /// to `'static`. Must be consistent with `static_type_of`.
    fn static_type_id() -> TypeId
    where
        Self: Sized;

    /// Must return the `TypeId` of `Self` but where the lifetimes are changed
    /// to `'static`. Must be consistent with `static_type_id`. Must not
    /// consult the `self` parameter in any way.
    fn static_type_of(&self) -> TypeId;
    // Required so we can have a `dyn AnyLifetime`.
}

impl<'a> dyn AnyLifetime<'a> {
    /// Is the value of type `T`.
    pub fn is<T: AnyLifetime<'a>>(&self) -> bool {
        self.static_type_of() == T::static_type_id()
    }

    /// Downcast a reference to type `T`, or return [`None`](None) if it is not the
    /// right type.
    pub fn downcast_ref<T: AnyLifetime<'a>>(&self) -> Option<&T> {
        if self.is::<T>() {
            // SAFETY: just checked whether we are pointing to the correct type.
            unsafe { Some(&*(self as *const Self as *const T)) }
        } else {
            None
        }
    }

    /// Downcast a mutable reference to type `T`, or return [`None`](None) if it is not
    /// the right type.
    pub fn downcast_mut<T: AnyLifetime<'a>>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            // SAFETY: just checked whether we are pointing to the correct type.
            unsafe { Some(&mut *(self as *mut Self as *mut T)) }
        } else {
            None
        }
    }
}

#[macro_export]
/// Used to implement the [`AnyLifetime` trait](crate::any::AnyLifetime).
///
/// Consider implementing `ProvidesStaticType` instead.
macro_rules! any_lifetime_body {
    ( $t:ty ) => {
        fn static_type_id() -> std::any::TypeId {
            std::any::TypeId::of::<$t>()
        }
        fn static_type_of(&self) -> std::any::TypeId {
            std::any::TypeId::of::<$t>()
        }
    };
}

#[macro_export]
/// Used to implement the [`AnyLifetime` trait](crate::any::AnyLifetime).
///
/// Consider using `#[derive(AnyLifetime)]` or implementing `ProvidesStaticType` directly instead.
macro_rules! any_lifetime {
    ( $t:ident < $l:lifetime > ) => {
        unsafe impl<$l> $crate::any::ProvidesStaticType for $t<$l> {
            type StaticType = $t<'static>;
        }
    };
    ( & $t:ident ) => {
        unsafe impl<'l> $crate::any::ProvidesStaticType for &'l $t {
            type StaticType = &'static $t;
        }
    };
    ( $t:ty ) => {
        unsafe impl $crate::any::ProvidesStaticType for $t {
            type StaticType = $t;
        }
    };
}

// One of the disadvantages of AnyLifetime is there is no finite covering set of
// types so we predeclare instances for things that seem useful, but the list is
// pretty adhoc
any_lifetime!(());
any_lifetime!(bool);
any_lifetime!(u8);
any_lifetime!(u16);
any_lifetime!(u32);
any_lifetime!(u64);
any_lifetime!(u128);
any_lifetime!(usize);
any_lifetime!(i8);
any_lifetime!(i16);
any_lifetime!(i32);
any_lifetime!(i64);
any_lifetime!(i128);
any_lifetime!(isize);
any_lifetime!(f32);
any_lifetime!(f64);
any_lifetime!(String);
any_lifetime!(Box<str>);
any_lifetime!(&str);
any_lifetime!(str);

#[cfg(test)]
mod tests {
    use std::fmt::Display;

    use super::*;
    #[allow(unused_imports)] // Not actually unused, this makes testing the derive macro work
    use crate as gazebo;

    #[test]
    fn test_first_wins() {
        let mut r = AnyResult::new::<&'static str>();
        r.add(|| "a").add(|| "b");
        assert_eq!(r.result_ref::<&'static str>(), Some(&"a"));
        assert_eq!(r.result::<&'static str>(), Some("a"));
    }

    #[test]
    fn test_none() {
        let mut r = AnyResult::new::<String>();
        r.add(|| 1);
        assert_eq!(r.result_ref::<String>(), None);
        assert_eq!(r.result::<String>(), None);
    }

    #[test]
    #[should_panic(expected = "different types")]
    fn test_different_types() {
        AnyResult::new::<String>().result::<i32>();
    }

    #[test]
    fn test_can_convert() {
        #[derive(Debug, PartialEq, AnyLifetime)]
        struct Value<'a>(&'a str);

        #[derive(AnyLifetime)]
        struct Value2<'a>(&'a str);

        // Changing the return type too `Value<'static>` causes a compile error.
        fn convert_value<'a>(x: &'a Value<'a>) -> Option<&'a Value<'a>> {
            <dyn AnyLifetime>::downcast_ref(x)
        }

        fn convert_any<'p, 'a>(x: &'p dyn AnyLifetime<'a>) -> Option<&'p Value<'a>> {
            x.downcast_ref()
        }

        let v = Value("test");
        let v2 = Value2("test");
        assert_eq!(convert_value(&v), Some(&v));
        assert_eq!(convert_any(&v), Some(&v));
        assert_eq!(convert_any(&v2), None);
    }

    #[test]
    fn test_provides_static_type_id() {
        fn test<'a, A: AnyLifetime<'a>>(expected: TypeId) {
            assert_eq!(expected, A::static_type_id());
        }

        #[derive(AnyLifetime)]
        struct Aaa;
        test::<Aaa>(TypeId::of::<Aaa>());

        #[derive(AnyLifetime)]
        struct Bbb<'a>(&'a str);
        test::<Bbb>(TypeId::of::<Bbb<'static>>());

        #[derive(AnyLifetime)]
        struct Bbb2<'a, 'b>(&'a str, &'b str);
        test::<Bbb2>(TypeId::of::<Bbb2<'static, 'static>>());

        #[derive(AnyLifetime)]
        struct Ccc<X>(X);
        test::<Ccc<String>>(TypeId::of::<Ccc<String>>());

        #[derive(AnyLifetime)]
        struct LifetimeTypeConst<'a, T, const N: usize>([&'a T; N]);
        test::<LifetimeTypeConst<i32, 3>>(TypeId::of::<LifetimeTypeConst<'static, i32, 3>>());

        #[derive(AnyLifetime)]
        struct TypeWithConstraint<T: Display>(T);
        test::<TypeWithConstraint<String>>(TypeId::of::<TypeWithConstraint<String>>());
    }
}
