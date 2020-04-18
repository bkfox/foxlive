use proc_macro::TokenStream;



mod object;
mod service;
mod utils;


#[proc_macro_attribute]
pub fn object(a: TokenStream, i: TokenStream) -> TokenStream {
    object::object(a, i)
}

#[proc_macro_attribute]
pub fn service(a: TokenStream, i: TokenStream) -> TokenStream {
    service::service(a, i)
}


