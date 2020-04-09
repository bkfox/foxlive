//! Provides procedural macro to implement libfoxlive::Controller to a struct
//!
//! # Example
//!
//! ```
//! #[derive(Controller)]
//! struct MyController {
//!     a: i32,
//!     #[control(ControllerType::Range(-1,1,0.1))]
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


/// Read "control" attribute, returning it as `(type, name)`.
/// Attribute format: `#[control(type, label?, get_func, set_func)]`
fn read_control_attr(field: String, attr: &Attribute) -> Option<(Expr, String, Expr, Expr)> {
    if !attr.path.is_ident("control") {
        return None;
    }
    if let AttrStyle::Outer = attr.style {
        if let Meta::List(meta) = attr.parse_meta().unwrap() {
            let args = meta.nested;
            if let syn::NestedMeta::Meta(ref meta) = args[0] {
                let meta = meta.to_token_stream();

                // control type
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
                                set = parse_str::<Expr>(&format!("self.{}(value).map(|r| r.into())", ident)).ok();
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


/// Return control informations for the provided field as: `field, control_type, get, set, metas`.
fn get_control_info(field: &mut Field) -> Option<(Ident, Expr, Expr, Expr, Expr)> {
    let field_ident = field.ident.as_ref().unwrap();

    let mut infos = None;
    let mut metas = String::new();

    drain_attrs(&mut field.attrs, |attr| {
        if let Some((c_type_, name, get, set)) = read_control_attr(field_ident.to_string(), attr) {
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


/// Get all controls info as unzipped lists of elements (because quote does not access
/// value fields)
fn get_controls(data_struct: &mut DataStruct)
    -> (Vec<syn::Index>, Vec<Ident>, Vec<Expr>, Vec<Expr>, Vec<Expr>, Vec<Expr>)
{
    let controls = data_struct.fields.iter_mut().filter_map(get_control_info);
    let (mut a, mut b, mut c, mut d, mut e, mut f) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());

    controls.enumerate().for_each(|(index, control)| {
        a.push(syn::Index::from(index));
        b.push(control.0);
        c.push(control.1);
        d.push(control.2);
        e.push(control.3);
        f.push(control.4);
    });

    (a, b, c, d, e, f)
}


/// Return controller informations as: `metadata`.
/// #[foxlive_controller(label?)]
fn get_controller(attrs: TokenStream, ast: &mut DeriveInput) -> Expr {
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


#[proc_macro_attribute]
pub fn foxlive_controller(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let mut ast = parse::<DeriveInput>(input).unwrap();

    let controller_meta = get_controller(attrs, &mut ast);
    let name = &ast.ident;

    let data_struct = match &mut ast.data {
        syn::Data::Struct(ref mut ds) => ds,
        _ => panic!("controller only for struct"),
    };
    let (fields_index, fields, fields_control, fields_get, fields_set, fields_meta) = get_controls(data_struct);

    let impl_mod_name = parse_str::<syn::Ident>(&format!("impl_controller_for_{}", name.to_string())).unwrap();
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let expanded = quote! {
        #ast

        mod #impl_mod_name {
            use super::*;
            use libfoxlive::dsp::controller::*;

            impl #impl_generics Controller for #name #ty_generics #where_clause {
                fn get_metadata(&mut self) -> Metadatas {
                    #controller_meta
                }

                fn get_control(&self, control: ControlIndex) -> Option<ControlValue> {
                    use std::convert::TryInto;
                    match control {
                        #(#fields_index => Some(#fields_get),)*
                        _ => None,
                    }
                }

                fn set_control(&mut self, control: ControlIndex, value: ControlValue) -> Result<ControlValue, ()> {
                    use std::convert::TryInto;
                    match control {
                        #(#fields_index => { 
                            if let Ok(value) = value.try_into() {
                                #fields_set
                            } else { Err(()) }
                        }),*
                        _ => Err(()),
                    }
                }

                fn map_controls(&self, mapper: &mut dyn ControlsMapper) {
                    #(mapper.declare(#fields_index, ControlType::#fields_control, #fields_meta);)*
                }
            }
        }
    };

    // Hand the output tokens back to the compiler
    TokenStream::from(expanded)
}


