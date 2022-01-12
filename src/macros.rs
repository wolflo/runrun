// impl ChildTypesFn for Ctx0 { type Out = TList!(Ctx1, Ctx2); }
#[macro_export]
macro_rules! register_ctx {
    ($ctx:ty, [$($children:ty),*]) => (
        impl runrun::types::ChildTypesFn for $ctx {
            type Out = $crate::TList!($($children),*);
        }
    );
    ($ctx:ty) => {
        $crate::register_ctx!{$ctx, []}
    };
}


// TList macro from lloydmeta's frunk crate:
// https://github.com/lloydmeta/frunk/blob/09a3d4f45f7b2ac5b996fcdaa7c85173f0533ab1/core/src/macros.rs
#[macro_export]
macro_rules! TList {
    () => { $crate::types::TNil };
    (...$Rest:ty) => { $Rest };
    ($A:ty) => { $crate::TList![$A,] };
    ($A:ty, $($tok:tt)*) => {
        $crate::types::TCons<$A, $crate::TList![$($tok)*]>
    };
}
