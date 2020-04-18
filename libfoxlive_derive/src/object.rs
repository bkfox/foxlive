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
use syn;
use proc_macro::TokenStream;
use quote::{quote,ToTokens};
use syn::{parse, parse2, parse_str,
          Attribute, AttrStyle, DataStruct, DeriveInput,
          Expr, Field, Ident, Lit, Meta};


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


/// Read "field" attribute, returning it as `(type, name)`.
/// Attribute format: `#[field(type, label?, get_func, set_func)]`
fn read_field_attr(field: String, attr: &Attribute) -> Option<(Expr, String, Expr, Expr)> {
    if !attr.path.is_ident("field") {
        return None;
    }
    if let AttrStyle::Outer = attr.style {
        if let Meta::List(meta) = attr.parse_meta().unwrap() {
            let args = meta.nested;
            if let syn::NestedMeta::Meta(ref meta) = args[0] {
                let meta = meta.to_token_stream();

                // field type
                let c_type = parse2::<Expr>(meta).unwrap();

                let mut n = 1;

                // label?
                let mut label = String::new();
                if args.len() > n {
                    if let syn::NestedMeta::Lit(Lit::Str(ref label_)) = args[n] {
                        label = label_.value();
                        n += 1;
                    }
                }

                // getter?
                let mut get = None;
                let mut set = None;
                if args.len() > n {
                    if let syn::NestedMeta::Meta(ref meta) = args[n] {
                        let meta = meta.to_token_stream();
                        let ident = parse2::<Ident>(meta).unwrap().to_string();
                        get = parse_str::<Expr>(&format!("self.{}().into()", ident)).ok();
                        n += 1;

                        // set? (implies get)
                        if args.len() > n {
                            if let syn::NestedMeta::Meta(ref meta) = args[n] {
                                let meta = meta.to_token_stream();
                                let ident = parse2::<Ident>(meta).unwrap().to_string();
                                set = parse_str::<Expr>(&format!("self.{}(value).map(|r| r.into()).or(Err(()))", ident)).ok();
                                // n += 1;
                            }
                            else { panic!("invalid set argument"); }
                        }
                    }
                    else { panic!("invalid get argument"); }
                }

                if get.is_none(){
                    get = parse_str::<Expr>(&format!("self.{}.into()", field)).ok();
                }

                if set.is_none() {
                    set = parse_str::<Expr>(&format!("{{ self.{} = value; Ok(value.into()) }}", field)).ok();
                }

                return Some((c_type, label, get.unwrap(), set.unwrap()));
            }
        }
    }
    None
}


/// Read "meta" attribute, returning it as `(key, value)`.
/// Attribute format: `#[meta(key="value")]`.
fn read_meta_attr(attr: &Attribute) -> Option<String> {
    if let AttrStyle::Outer = attr.style {
        if attr.path.is_ident("meta") {
            if let Ok(Expr::Assign(expr)) = attr.parse_args::<Expr>() {
                if let (Expr::Path(left),Expr::Lit(right)) = (*expr.left, *expr.right) {
                    if let Lit::Str(right) = right.lit {
                        return Some(format!("(String::from(\"{}\"), String::from(\"{}\"))",
                            left.path.get_ident().unwrap().to_string(),
                            right.value())
                        )
                    }
                }
            }
        }
    }
    None
}


/// Return field informations as: `field, field_type, get, set, metas`.
fn get_field_info(field: &mut Field) -> Option<(Ident, Expr, Expr, Expr, Expr)> {
    let field_ident = field.ident.as_ref().unwrap();

    let mut infos = None;
    let mut metas = String::new();

    drain_attrs(&mut field.attrs, |attr| {
        if let Some((c_type_, name, get, set)) = read_field_attr(field_ident.to_string(), attr) {
            infos = Some((c_type_, get, set));
            if !name.is_empty() {
                metas += &format!("(String::from(\"name\"),String::from(\"{}\"))", name);
            }
            return true;
        }

        if let Some(meta) = read_meta_attr(attr) {
            metas += &meta;
            metas += ", ";
            return true;
        }
        false
    });

    infos.map(|(c_type, get, set)| {
        let metas = if metas.is_empty() { String::from("Vec::new()") }
                    else { format!("vec![{}]", metas) };
        (field_ident.clone(),
         c_type, get, set,
         syn::parse_str::<syn::Expr>(&metas).unwrap())
    })
}


/// Get all fields info as unzipped lists of elements (because quote does not access
/// value fields)
fn get_fields(data_struct: &mut DataStruct)
    -> (Vec<syn::Index>, Vec<Ident>, Vec<Expr>, Vec<Expr>, Vec<Expr>, Vec<Expr>)
{
    let fields = data_struct.fields.iter_mut().filter_map(get_field_info);
    let (mut a, mut b, mut c, mut d, mut e, mut f) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());

    fields.enumerate().for_each(|(index, field)| {
        a.push(syn::Index::from(index));
        b.push(field.0);
        c.push(field.1);
        d.push(field.2);
        e.push(field.3);
        f.push(field.4);
    });

    (a, b, c, d, e, f)
}


/// Return object informations as: `metadata`.
/// #[foxlive_object(label?)]
fn get_object(attrs: TokenStream, ast: &mut DeriveInput) -> Expr {
    let mut metas = String::new();

    // attrs
    if !attrs.is_empty() {
        let name = parse::<syn::LitStr>(attrs).unwrap();
        metas += &format!("(String::from(\"name\"),String::from(\"{}\")),", name.value());
    }

    // metadata
    drain_attrs(&mut ast.attrs, |attr| {
        if let Some(meta) = read_meta_attr(attr) {
            metas += &(meta + ",");
            true
        }
        else { false }
    });

    let metas = if metas.is_empty() { String::from("Metadatas::new()") }
                else { format!("vec![{}]", metas) };
    syn::parse_str::<syn::Expr>(&metas).unwrap()
}


pub fn object(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let mut ast = parse::<DeriveInput>(input).unwrap();

    let object_meta = get_object(attrs, &mut ast);
    let name = &ast.ident;

    let data_struct = match &mut ast.data {
        syn::Data::Struct(ref mut ds) => ds,
        _ => panic!("object only for struct"),
    };
    let (fields_index, _fields, fields_type, fields_get, fields_set, fields_meta) = get_fields(data_struct);

    let impl_mod_name = parse_str::<syn::Ident>(&format!("impl_object_for_{}", name.to_string())).unwrap();
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let expanded = quote! {
        #ast

        mod #impl_mod_name {
            use super::*;
            use libfoxlive::rpc::*;

            impl #impl_generics Object for #name #ty_generics #where_clause {
                fn object_meta(&self) -> ObjectMeta {
                    // TODO: label "name" or #name
                    ObjectMeta::new("#name", Some(#object_meta))
                }

                fn get_value(&self, index: ObjectIndex) -> Option<Value> {
                    use std::convert::TryInto;
                    match index {
                        #(#fields_index => Some(#fields_get),)*
                        _ => None,
                    }
                }

                fn set_value(&mut self, index: ObjectIndex, value: Value) -> Result<Value, ()> {
                    use std::convert::TryInto;
                    match index {
                        #(#fields_index =>
                            if let Ok(value) = value.try_into() {
                                #fields_set
                            } else { Err(()) }
                        ),*
                        _ => Err(()),
                    }
                }

                fn map_object(&self, mapper: &mut dyn ObjectMapper) {
                    #(mapper.declare(#fields_index, ValueType::#fields_type, #fields_meta);)*
                }
            }
        }
    };

    // Hand the output tokens back to the compiler
    TokenStream::from(expanded)
}

