use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::parse_macro_input;

#[proc_macro_attribute]
pub fn run_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);

    if input.sig.asyncness.is_none() { panic!("Non async test fn."); }

    let state_on = match input.sig.inputs.first().unwrap() {
        syn::FnArg::Typed(pat) => match &*pat.ty {
            syn::Type::Path(path) => &path.path.segments.first().unwrap().ident,
            _ => panic!(""),
        },
        _ => panic!(""),
    };
    let state_big = state_on.to_string().to_uppercase();

    let name = &input.sig.ident;


    let tests_id = format_ident!("TESTS_ON_{}", state_big);

    let res = quote! {
        const _: () = {
            #[linkme::distributed_slice(#tests_id)]
            static __: runrun::core::Testable<#state_on> = &runrun::core::TestCase { name: stringify!(#name), test: &|s| Box::pin(#name(s)) };
        };
        #input
    };
    res.into()
}

#[proc_macro_attribute]
pub fn run_ctx(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemImpl);

    let state_on = match &*input.self_ty {
        syn::Type::Path(path) => &path.path.segments.first().unwrap().ident,
        _ => panic!("Can't determine impl ctx."),
    };
    let state_big = state_on.to_string().to_uppercase();

    let tests_id = format_ident!("TESTS_ON_{}", state_big);

    let res = quote! {
        #[linkme::distributed_slice]
        pub static #tests_id: [runrun::core::Testable<#state_on>] = [..];

        impl runrun::core::TestSet for #state_on {
            fn tests() -> &'static [runrun::core::Testable<Self>] {
                &#tests_id
            }
        }
        #input
    };
    res.into()
}
