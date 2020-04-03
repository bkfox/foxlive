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
/// Attribute format: `#[control(type, label?)]`
fn read_control_attr(attr: &Attribute) -> Option<(Expr, String)> {
    if !attr.path.is_ident("control") {
        return None;
    }
    if let AttrStyle::Outer = attr.style {
        if let Meta::List(meta) = attr.parse_meta().unwrap() {
            let args = meta.nested;
            if let syn::NestedMeta::Meta(ref meta) = args[0] {
                let meta = meta.to_token_stream();
                let c_type = parse2::<Expr>(meta).unwrap();
                /*let c_type = meta.path().get_ident().unwrap().to_string();
                let c_type_args = meta.parse_args::<Expr>
                let c_type = syn::parse_str::<Expr>(&format!("ControlType::{}", c_type)).unwrap();*/

                if args.len() == 1 {
                    return Some((c_type, String::new()));
                }

                if let syn::NestedMeta::Lit(Lit::Str(ref label)) = args[1] {
                    return Some((c_type, label.value()));
                }
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


/// Return control informations for the provided field as: `field, control_type, metas`.
fn get_control_info(field: &mut Field) -> Option<(Ident, Expr, Expr)> {
    let mut c_type = None;
    let mut metas = String::new();
    drain_attrs(&mut field.attrs, |attr| {
        if let Some((c_type_, name)) = read_control_attr(attr) {
            c_type = Some(c_type_);
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

    c_type.map(|c_type| {
        let metas = if metas.is_empty() { String::from("Vec::new()") }
                    else { format!("vec![{}]", metas) };
        (field.ident.as_ref().unwrap().clone(),
         c_type,
         syn::parse_str::<syn::Expr>(&metas).unwrap())
    })
}


/// Get all controls info as unzipped lists of elements (because quote does not access
/// value fields)
fn get_controls(data_struct: &mut DataStruct)
    -> (Vec<syn::Index>, Vec<Ident>, Vec<Expr>, Vec<Expr>)
{
    let controls = data_struct.fields.iter_mut().filter_map(get_control_info);
    let (mut a, mut b, mut c, mut d) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());

    controls.enumerate().for_each(|(index, control)| {
        a.push(syn::Index::from(index));
        b.push(control.0);
        c.push(control.1);
        d.push(control.2);
    });

    (a, b, c, d)
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
    let (fields_index, fields, fields_control, fields_meta) = get_controls(data_struct);

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
                        #(#fields_index => self.#fields.try_into().ok(),)*
                        _ => None,
                    }
                }

                fn set_control(&mut self, control: ControlIndex, value: ControlValue) -> Result<ControlValue, ()> {
                    use std::convert::TryInto;
                    match control {
                        #(#fields_index => { 
                            if let Ok(v) = value.try_into() {
                                self.#fields = v;
                                Ok(value)
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


