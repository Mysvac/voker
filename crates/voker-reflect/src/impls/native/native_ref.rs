use crate::{impls::GenericTypePathCell, info::TypePath};

impl<T: TypePath + ?Sized> TypePath for &'static T {
    fn type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self>(|| crate::impls::concat(&["&", T::type_path()]))
    }

    fn type_name() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self>(|| crate::impls::concat(&["&", T::type_name()]))
    }

    fn type_ident() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self>(|| crate::impls::concat(&["&", T::type_ident()]))
    }
}

impl<T: TypePath + ?Sized> TypePath for &'static mut T {
    fn type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self>(|| crate::impls::concat(&["&mut ", T::type_path()]))
    }

    fn type_name() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self>(|| crate::impls::concat(&["&mut ", T::type_name()]))
    }

    fn type_ident() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self>(|| crate::impls::concat(&["&mut ", T::type_ident()]))
    }
}
