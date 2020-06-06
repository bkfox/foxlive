extern crate proc_macro;

use std::convert::From;
use syn;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote,ToTokens};

use super::utils::*;


struct Service<'a> {
    ast: &'a syn::ItemImpl,
    idents: Vec<syn::Ident>,
    idents_cap: Vec<syn::Ident>,
    args: Vec<Vec<syn::Pat>>,
    args_ty: Vec<Vec<syn::Type>>,
    outputs: Vec<Option<syn::Type>>,
}

impl<'a> Service<'a> {
    pub fn new(ast: &'a syn::ItemImpl) -> Self {
        let signatures = ast.items.iter().filter_map(|item| match item {
            syn::ImplItem::Method(item) => Some(&item.sig),
            _ => None,
        });

        let (mut idents, mut idents_cap, mut args, mut args_ty, mut outputs) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for sig in signatures {
            let (mut a, mut a_t) = (Vec::new(), Vec::new());
            let mut has_self = false;
            for arg in sig.inputs.iter() {
                match arg {
                    syn::FnArg::Typed(arg) => {
                        a.push((*arg.pat).clone());
                        a_t.push((*arg.ty).clone());
                    },
                    syn::FnArg::Receiver(_) => {
                        has_self = true;
                    },
                }
            }

            if !has_self {
                continue;
            }

            let ident = sig.ident.clone();
            args.push(a);
            args_ty.push(a_t);
            idents_cap.push(to_camel_ident(&ident));
            idents.push(ident);
            outputs.push(match sig.output.clone() {
                syn::ReturnType::Default => None,
                syn::ReturnType::Type(_, ty) => Some(*ty),
            });
            //sigs.push(sig.clone());
        }

        Self { ast: &ast, idents, idents_cap, args, args_ty, outputs }
    }

    pub fn generate(&self) -> TokenStream {
        let ast = &self.ast;
        let (types, server, client) = (self.types(), self.server(), self.client());

        (quote!{
            #ast

            pub mod service {
                use super::*;
                use libfoxlive::rpc::Service;
                use std::marker::PhantomData;
                use futures::future::{Future,FutureExt,ok,err};

                #types
                #server
                #client
            }
        }).into()
    }

    fn types(&self) -> TokenStream2 {
        let Self { idents_cap, args_ty, outputs, .. } = self;
        let (_impl_generics, ty_generics, where_clause) = self.ast.generics.split_for_impl();

        // we need phantom variant for handling generics cases: R, R<A>, R<A,B>.
        let phantom = quote! { _Phantom(PhantomData<Request #ty_generics>) };

        let responses = outputs.iter().zip(idents_cap).map(|(output, ident)| match output {
            None => quote! { #ident },
            Some(t) => quote! { #ident(#t) },
        });

        // Response as future
        /* let fut_outputs = outputs.iter().zip(sigs.iter()).map(|(output, sig)| {
            if sig.asyncness.is_some() {
                panic!(format!("{}", output.to_token_stream()))
            } else {
            }
        }).collect::<Vec<_>>(); */

        quote! {
            pub enum Request #ty_generics #where_clause {
                #(#idents_cap(#(#args_ty),*),)*
                #phantom
            }

            #[derive(Clone)]
            pub enum Response #ty_generics #where_clause {
                #(#responses,)*
                #phantom
            }
        }
    }

    fn server(&self) -> TokenStream2 {
        let Self { ast, idents, idents_cap, args, outputs, .. } = self;
        let ty = &*ast.self_ty;
        let (impl_generics, ty_generics, where_clause) = self.ast.generics.split_for_impl();

        let calls = outputs.iter().enumerate().map(|(i, output)| {
            let (ident, ident_cap, args) = (&idents[i], &idents_cap[i], &args[i]);
            match output {
                None => quote! {{
                    self.#ident(#(#args),*);
                    Some(Response::#ident_cap)
                }},
                Some(_) => quote! { Some(Response::#ident_cap(self.#ident(#(#args),*))) },
            }
        });

        quote! {
            impl #impl_generics Service for #ty #where_clause {
                type Request = Request #ty_generics;
                type Response = Response #ty_generics;
                // type ResponseFut = ResponseFut #impl_generics;

                fn process_request(&mut self, request: Self::Request) -> Option<Self::Response> {
                    match request {
                        #(Request::#idents_cap(#(#args),*) => #calls,)*
                        _ => None,
                    }
                }
            }
        }
    }

    fn client(&self) -> TokenStream2 {
        let Self { idents, idents_cap, args, args_ty, outputs, .. } = self;

        let generics = self.ast.generics.clone();
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        let variants = outputs.iter().zip(idents_cap).map(|(output, ident)| match output {
            None => quote! { Ok(Response::#ident) => ok(()) },
            Some(_) => quote! { Ok(Response::#ident(r)) => ok(r) },
        });
        let outputs = outputs.iter().map(|o| match o {
            None => quote! { () },
            Some(t) => t.to_token_stream(),
        });


        quote! {
            pub trait Client #impl_generics #where_clause {
                type ResponseFut : Future<Output=Result<Response #ty_generics,()>>+'static;

                fn send_request(&mut self, request: Request #ty_generics) -> Self::ResponseFut;

                #(fn #idents(&mut self, #(#args: #args_ty),*) -> Box<Future<Output=Result<#outputs,()>>> {
                    Box::new(self.send_request(Request::#idents_cap(#(#args),*))
                        .then(|response| match response {
                            #variants,
                            _ => err(())
                        }))
                })*
            }
        }
    }
}


/// Macro generating RPC service traits and types, for the decorated
/// struct impl block.
pub fn service(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let ast = syn::parse::<syn::ItemImpl>(input).unwrap();
    let service = Service::new(&ast);
    service.generate()
}

