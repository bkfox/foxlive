use proc_macro::TokenStream;


mod object;
mod service;
mod utils;


#[proc_macro_attribute]
pub fn object(a: TokenStream, i: TokenStream) -> TokenStream {
    object::object(a, i)
}

/// Generates RPC service and related classes around a server-side `impl` block of RPC methods.
///
/// The code is generated inside the `service` module:
/// - `Client` trait: client implementation to call RPC, mapping service's RPC methods. Only
///     `send_request(&mut self, request: Request)` must be implemented by user.
/// - `Request`, `Response` enums: a variant for each RPC method. They have same generics as
/// Service.
/// - Implementaton of `Service` trait for the struct implementing RPC methods;
///
///
/// # Example
///
/// ```
/// struct ExampleService {
///     channel: MPSCChannel<service::Response, service::Request>,
/// }
///
/// struct ExampleClient {
///     channel: MPSCChannel<service::Request, service::Response>,
/// }
///
/// #[service]
/// impl ExampleService {
///     fn echo(text: String) -> String {
///         text.graphemes(true).rev().collect()
///     }
///
///     fn add(a: u32, b: u32) -> u32 {
///         a+b
///     }
/// }
///
/// impl service::Client for ExampleClient {
///     // FIXME: type ResponseFut
///
///     fn send_request(&mut self, request: Request #ty_generics) -> Self::ResponseFut {
///         self.channel.sender.send(request)
///     }
/// }
///
/// ```
///
#[proc_macro_attribute]
pub fn service(a: TokenStream, i: TokenStream) -> TokenStream {
    service::service(a, i)
}


