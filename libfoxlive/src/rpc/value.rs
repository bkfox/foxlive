//! A Value is used in order to interface with different user interfaces and their components.
use std::convert::TryFrom;
use std::time::Duration;


pub trait IntoValue : TryFrom<Value>+Into<Value> {}

macro_rules! ValueEnum {
    (into_value: $variant:ident => $type:ty) => {
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

    };

    ($($variant:ident => $type:ty $(| $info:ident)?),*) => {
        pub enum ValueType {
            $($variant),*
        }

        #[derive(Clone,Debug)]
        pub enum Value {
            $($variant($type)),*
        }

        impl Value {
            fn get_type(&self) -> ValueType {
                match self {
                    $(Self::$variant(_) => ValueType::$variant),*
                }
            }
        }
        $(ValueEnum!{into_value: $variant => $type})*
    };
}


macro_rules! RangeEnum {
    ($($variant:ident => $type:ty $(| $info:ident)?),*) => {
        pub enum Range {
            $($variant($type,$type,$type)),*
        }

        impl Range {
            fn get_type(&self) -> ValueType {
                match self {
                    $(Self::$variant(_,_,_) => ValueType::$variant),*
                }
            }

            fn min(&self) -> Value {
                match self {
                    $(Self::$variant(a,_,_) => Value::$variant(*a)),*
                }
            }

            fn max(&self) -> Value {
                match self {
                    $(Self::$variant(_,a,_) => Value::$variant(*a)),*
                }
            }

            fn step(&self) -> Value {
                match self {
                    $(Self::$variant(_,_,a) => Value::$variant(*a)),*
                }
            }
        }

        // TODO: IntoRange => tuple
    }

}

ValueEnum!{
    Bool => bool, U8 => u8, I16 => i16, I32 => i32, F32 => f32, F64 => f64,
    Duration => Duration, Index => usize, String => String
}


RangeEnum!{
    U8 => u8, I16 => i16, I32 => i32, F32 => f32, F64 => f64
}


