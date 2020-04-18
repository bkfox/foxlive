//! A Value is used in order to interface with different user interfaces and their components.
use std::convert::TryFrom;
use std::time::Duration;


// TODO: optional range value with f32::MIN_FLOAT or whatever
/// Control type. Some variant have tuple values that specify range value as `(min, max, step)`.
#[derive(Copy,Clone,Debug)]
pub enum ValueType {
    Bool,
    U8(u8,u8,u8),
    I16(i16,i16,i16),
    I32(i32,i32,i32),
    F32(f32,f32,f32),
    F64(f64,f64,f64),
    Duration,
    Index,
    String,
}


/// A generic value that can be passed down to controller
#[derive(Clone,Debug)]
pub enum Value {
    Bool(bool),
    U8(u8),
    I16(i16),
    I32(i32),
    F32(f32),
    F64(f64),
    Duration(Duration),
    Index(usize),
    String(String),
    // SelectionList
}


pub trait IntoValue : TryFrom<Value>+Into<Value> {}


macro_rules! ImplValue {
    ($variant:ident, $type:ident) => {
        impl TryFrom<Value> for $type {
            type Error = ();

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                match value {
                    Value::$variant(v) => Ok(v),
                    _ => Err(()),
                }
            }
        }

        impl Into<Value> for $type {
            fn into(self) -> Value {
                Value::$variant(self)
            }
        }

        impl IntoValue for $type {}
    }
}

ImplValue!{ Bool, bool }
ImplValue!{ U8, u8 }
ImplValue!{ I16, i16 }
ImplValue!{ I32, i32 }
ImplValue!{ F32, f32 }
ImplValue!{ F64, f64 }
ImplValue!{ Duration, Duration }
ImplValue!{ Index, usize }
ImplValue!{ String, String }



