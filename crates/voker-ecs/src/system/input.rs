/// Trait for types that can be used as system inputs.
///
/// `SystemInput` defines how input values are passed into systems and how they
/// can be composed. It separates the "borrowed" form (`Data`) used during system
/// execution from the "owned" form (`Item`) used for storage and wrapping.
///
/// # Implementations
///
/// The crate provides implementations for:
/// - Unit type `()` (no input)
/// - [`In<T>`] for owned values
/// - [`InRef<'_, T>`] for shared references
/// - [`InMut<'_, T>`] for mutable references
/// - Tuples of types that implement `SystemInput` (up to 12 elements)
///
/// # Examples
///
/// ```ignore
/// fn system_a(/* .. */) -> String { /* .. */ }
/// fn system_b(input: In<String>) { /* .. */ }
/// let system_c = system_a.pipe(system_b);
///
/// fn system_e(/* .. */) -> (i32, &Foo, &mut Bar) { /* .. */ }
/// fn system_f(input: (In<i32>, InRef<Foo>, InMut<Bar>)) { /* .. */ }
/// let system_g = system_e.pipe(system_f);
/// ```
pub trait SystemInput: Sized {
    /// The borrowed data type passed to system execution.  
    type Data<'i>;
    /// The wrapper type that implements `SystemInput` for storage.
    type Item<'i>: SystemInput;

    fn wrap(this: Self::Data<'_>) -> Self::Item<'_>;
}

#[derive(Debug)]
#[repr(transparent)]
pub struct In<T>(pub T);

impl<T: 'static> SystemInput for In<T> {
    type Data<'i> = T;
    type Item<'i> = In<T>;

    #[inline(always)]
    fn wrap(this: Self::Data<'_>) -> Self::Item<'_> {
        In(this)
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct InRef<'i, T: ?Sized>(pub &'i T);

impl<T: ?Sized + 'static> SystemInput for InRef<'_, T> {
    type Data<'i> = &'i T;
    type Item<'i> = InRef<'i, T>;

    #[inline(always)]
    fn wrap(this: Self::Data<'_>) -> Self::Item<'_> {
        InRef(this)
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct InMut<'a, T: ?Sized>(pub &'a mut T);

impl<T: ?Sized + 'static> SystemInput for InMut<'_, T> {
    type Data<'i> = &'i mut T;
    type Item<'i> = InMut<'i, T>;

    #[inline(always)]
    fn wrap(this: Self::Data<'_>) -> Self::Item<'_> {
        InMut(this)
    }
}

macro_rules! impl_tuple {
    (0: []) => {
        impl SystemInput for () {
            type Data<'i> = ();
            type Item<'i> = ();

            #[inline(always)]
            fn wrap(_: Self::Data<'_>) -> Self::Item<'_> {}
        }
    };
    (1 : [ $index:tt : $name:ident ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 8 items long.")]
        impl<$name: SystemInput> SystemInput for ($name,) {
            type Data<'i> = ( <$name>::Data<'i>, );
            type Item<'i> = ( <$name>::Item<'i>, );

            fn wrap(this: Self::Data<'_>) -> Self::Item<'_> {
                ( <$name as SystemInput>::wrap(this.0), )
            }
        }
    };
    ($num:literal : [$($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        impl<$($name: SystemInput),*> SystemInput for ($($name),*) {
            type Data<'i> = ( $( <$name>::Data<'i> ),* );
            type Item<'i> = ( $( <$name>::Item<'i> ),* );

            fn wrap(this: Self::Data<'_>) -> Self::Item<'_> {
                ( $( <$name as SystemInput>::wrap(this.$index) ),* )
            }
        }
    };
}

voker_utils::range_invoke!(impl_tuple, 8);
