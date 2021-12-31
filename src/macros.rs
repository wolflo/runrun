// All credit to lloydmeta's frunk crate:
// https://github.com/lloydmeta/frunk/blob/09a3d4f45f7b2ac5b996fcdaa7c85173f0533ab1/core/src/macros.rs

#[macro_export]
macro_rules! tlist {
    () => { $crate::ty::TNil };
    (...$rest:expr) => { $rest };
    ($a:expr) => { $crate::tlist![$a,] };
    ($a:expr, $($tok:tt)*) => {
        $crate::ty::TCons {
            head: $a,
            tail: $crate::tlist![$($tok)*],
        }
    };
}

#[macro_export]
macro_rules! TList {
    () => { $crate::ty::TNil };
    (...$Rest:ty) => { $Rest };
    ($A:ty) => { $crate::TList![$A,] };
    ($A:ty, $($tok:tt)*) => {
        $crate::ty::TCons<$A, $crate::TList![$($tok)*]>
    };
}
