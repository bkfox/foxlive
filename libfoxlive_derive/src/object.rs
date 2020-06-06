//! Provides procedural macro to implement libfoxlive::Object to a struct
//!
//! # Example
//!
//! ```
//! #[derive(Object)]
//! struct MyObject {
//!     a: i32,
//!     #[field(ObjectType::Range(-1,1,0.1))]
//!     b: f32,
//! }
//! ```
//!
extern crate proc_macro;

use std::convert::From;
use proc_macro::{TokenStream};
use proc_macro2::{TokenStream as TokenStream2};
use quote::{quote,ToTokens};
use syn;
use syn::{parse, parse2, parse_str,
          Attribute, DeriveInput,
          Expr, Ident};


/// Run over attributes with the provided function, removing attribute when `func` returns `true`.
fn drain_attrs(attrs: &mut Vec<Attribute>, mut func: impl FnMut(&Attribute) -> bool)
{
    let mut i = 0;
    while i != attrs.len() {
        if func(&attrs[i]) {
            attrs.remove(i);
        }
        else { i += 1 }
    }
}


/// Object or Field metadatas, as `[meta("key","value")]`
struct Metadatas {
    items: Vec<syn::Expr>,
}

impl Metadatas {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Add metadata from Expr.
    fn push(&mut self, expr: syn::Expr) {
        self.items.push(expr);
    }

    /// Add a metadata from attribute, return true if it has been taken.
    fn push_attr(&mut self, attr: &syn::Attribute) -> bool {
        if let syn::AttrStyle::Inner(_) = attr.style {
            return false;
        }
        if !attr.path.is_ident("meta") {
            return false;
        }

        self.push(parse2::<syn::Expr>(attr.tokens.clone())
                      .expect("can not parse meta declaration"));
        true
    }

    /// Add metadata from key and value
    fn insert(&mut self, key: &str, value: String) {
        self.push(parse_str::<syn::Expr>(&format!("(\"{}\", \"{}\")", key, value)).
                    unwrap())
    }

    fn render(&self) -> TokenStream2 {
        let items = &self.items;
        quote! { vec![#(#items),*] }
    }
}



/// Declaration an object's field as:
///     `[field("label", I32(0.1), range(0,0,1),? get(getter),? set(setter)?)]`
///
/// Where:
/// - `"label"`: field's human-readable label
/// - `I32(0.1)`: field type and default value
/// - `range(...)`: value range
/// - `get`: specifies a getter method
/// - `set`: specifies a setter method
struct FieldDecl {
    ident: Ident,
    value_type: Expr,
    default: Option<syn::punctuated::Punctuated<Expr,syn::token::Comma>>,
    range: Option<syn::punctuated::Punctuated<Expr,syn::token::Comma>>,
    // metadatas as vec of ["(String::from("key"),String::from("value")]
    metadatas: Metadatas,
    get: Expr,
    set: Expr,
}

impl FieldDecl {
    fn new(field: &mut syn::Field) -> Option<Self> {
        let (attr_i, attr) = match field.attrs.iter().enumerate().find(|(i, attr)| attr.path.is_ident("field")) {
            Some(attr) => attr,
            None => return None,
        };

        let decl = parse2::<syn::ExprTuple>(attr.tokens.clone())
                    .expect("can not parse field declaration");
        let mut args = decl.elems.iter();

        // label
        let label = match args.next().expect("missing field label") {
            syn::Expr::Lit(ref lit) => match lit.lit {
                syn::Lit::Str(ref lit) => lit.value(),
                _ => panic!("expected label string"),
            },
            _ => panic!("invalid field label"),
        };

        let value_type = args.next().expect("missing field type").clone();
        let (value_type, default) = match value_type {
            Expr::Call(ref arg) => (*arg.func.clone(), Some(arg.args.clone())),
            _ => (value_type, None),
        };

        let (mut range, mut get, mut set) = (None, None, None);
        for arg in args {
            if let Expr::Call(arg) = arg {
                if let Expr::Path(path) = arg.func.as_ref() {
                    let ident = path.path.get_ident()
                                    .expect("invalid argument ident {:?}")
                                    .to_string();
                    match &ident[..] {
                        "get" => { get = Some(arg.args.first()
                                                 .expect("missing `get` argument").clone()
                                                 .to_token_stream().to_string()); },
                        "set" => { set = Some(arg.args.first()
                                                 .expect("missing `set` argument").clone()
                                                 .to_token_stream().to_string()); },
                        "range" => { range = Some(arg.args.clone());
                        },
                        _ => panic!("invalid argument {}", ident),
                    }
                }
            }
        }

        // metadatas
        let mut metadatas = Metadatas::new();
        metadatas.insert("label", label);
        drain_attrs(&mut field.attrs, |attr| metadatas.push_attr(attr));

        // finalize
        let ident = field.ident.as_ref().unwrap().clone();
        let ident_str = ident.to_string();

        field.attrs.remove(attr_i);

        Some(Self {
            ident, value_type, default, range, metadatas,
            get: parse_str::<Expr>(&format!("self.{}.into()", get
                    .map(|v| v + "()")
                    .unwrap_or_else(|| ident_str.clone())))
                    .unwrap(),
            set: parse_str::<Expr>(&set
                    .map(|v| format!("self.{}(value).map(|r| r.into()).or(Err(()))", v))
                    .unwrap_or_else(|| format!("{{ self.{} = value; Ok(value.into()) }}", ident_str.clone()))
                    ).unwrap(),
        })
    }

    fn render_map(&self, index: usize) -> TokenStream2 {
        let Self { value_type, default, range, metadatas, .. } = self;

        let default = match default {
            Some(args) => quote! { Some(Value::#value_type(#args)) },
            None => quote! { None }
        };
        let range = match range {
            Some(args) => quote! { Some(Range::#value_type(#args)) },
            None => quote! { None }
        };
        let metadatas = metadatas.render();

        quote! {
            mapper.declare(FieldInfo {
                index: #index as ObjectIndex,
                value_type: ValueType::#value_type,
                default: #default, range: #range,
                metadatas: #metadatas
            });
        }
    }
}



struct Object {
    ast: syn::DeriveInput,
    metadatas: Metadatas,
    fields: Vec<FieldDecl>,
}

impl Object {
    pub fn new(attrs: TokenStream, mut ast: syn::DeriveInput) -> Self {
        let metadatas = Self::get_metadatas(attrs, &mut ast);
        let mut obj = Object {
            ast, metadatas, fields: Vec::new(),
        };
        obj.get_fields();
        obj
    }

    /// Read metadatas
    fn get_metadatas(attrs: TokenStream, ast: &mut syn::DeriveInput) -> Metadatas {
        let mut metadatas = Metadatas::new();

        if !attrs.is_empty() {
            // provided label
            let label = parse::<syn::LitStr>(attrs).unwrap();
            metadatas.insert("label", label.value());
        }

        drain_attrs(&mut ast.attrs, |attr| metadatas.push_attr(attr));
        metadatas
    }

    /// Read a fields declarations and attributes
    fn get_fields(&mut self) {
        let ds = match &mut self.ast.data {
            syn::Data::Struct(ref mut ds) => ds,
            _ => panic!("can't read object as struct"),
        };

        for field in ds.fields.iter_mut() {
            if let Some(field) = FieldDecl::new(field) {
                self.fields.push(field);
            }
        }
    }

    pub fn render(&self) -> TokenStream {
        let ast = &self.ast;

        let name = &ast.ident;
        let impl_mod_name = parse_str::<syn::Ident>(&format!("impl_object_for_{}", &name.to_string())).unwrap();
        let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

        let metadatas = &self.metadatas.render();
        let f_map = self.fields.iter().enumerate().map(|(i, f)| f.render_map(i));
        let (f_index, f_get, f_set) = (
            (0..self.fields.len()).collect::<Vec<_>>(),
            self.fields.iter().map(|f| &f.get).collect::<Vec<_>>(),
            self.fields.iter().map(|f| &f.set).collect::<Vec<_>>(),
        );

        let expanded = quote! {
            #ast

            mod #impl_mod_name {
                use super::*;
                use libfoxlive::rpc::*;

                impl #impl_generics Object for #name #ty_generics #where_clause {
                    fn object_meta(&self) -> ObjectMeta {
                        // TODO: label "name" or #name
                        ObjectMeta::new("#name", Some(#metadatas))
                    }

                    fn get_value(&self, index: ObjectIndex) -> Option<Value> {
                        use std::convert::TryInto;
                        match index as usize {
                            #(#f_index => Some(#f_get),)*
                            _ => None,
                        }
                    }

                    fn set_value(&mut self, index: ObjectIndex, value: Value) -> Result<Value, ()> {
                        use std::convert::TryInto;
                        match index as usize {
                            #(#f_index =>
                                if let Ok(value) = value.try_into() {
                                    #f_set
                                } else { Err(()) }
                            ),*
                            _ => Err(()),
                        }
                    }

                    fn map_object(&self, mapper: &mut dyn ObjectMapper) {
                        #(#f_map)*
                    }
                }
            }
        };

        // Hand the output tokens back to the compiler
        TokenStream::from(expanded)
    }
}


pub fn object(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let ast = parse::<DeriveInput>(input).expect("can not parse input");
    let object = Object::new(attrs, ast);
    object.render()
}

