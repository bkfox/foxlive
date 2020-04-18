use std::convert::TryFrom;
use std::time::Duration;

use crate::data::{NSamples};

pub type ControlIndex = u32;


/// Control type. Some variant have tuple values that specify range value as `(min, max, step)`.
#[derive(Copy,Clone,Debug)]
pub enum ControlType {
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
pub enum ControlValue {
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


pub trait IntoValue : TryFrom<ControlValue>+Into<ControlValue> {}


macro_rules! ImplControlValue {
    ($variant:ident, $type:ident) => {
        impl TryFrom<ControlValue> for $type {
            type Error = ();

            fn try_from(value: ControlValue) -> Result<Self, Self::Error> {
                match value {
                    ControlValue::$variant(v) => Ok(v),
                    _ => Err(()),
                }
            }
        }

        impl Into<ControlValue> for $type {
            fn into(self) -> ControlValue {
                ControlValue::$variant(self)
            }
        }

        impl IntoValue for $type {}
    }
}

ImplControlValue!{ Bool, bool }
ImplControlValue!{ U8, u8 }
ImplControlValue!{ I16, i16 }
ImplControlValue!{ I32, i32 }
ImplControlValue!{ F32, f32 }
ImplControlValue!{ F64, f64 }
ImplControlValue!{ Duration, Duration }
ImplControlValue!{ Index, usize }
ImplControlValue!{ String, String }


/// Metadata as (key, value)
pub type Metadata = (String,String);
/// List of metadatas for controller and its controls
pub type Metadatas = Vec<Metadata>;

/// Map information to a control
pub struct ControlMap {
    pub control: ControlIndex,
    pub control_type: ControlType,
    pub metadata: Metadatas,
}


/// Trait providing interface to declare controls
pub trait ControlsMapper {
    /// Add a new control to the controls' map
    fn declare(&mut self, control: ControlIndex, control_type: ControlType,
               metadata: Metadatas);
}


/// A Controller provides mapping to some of its values in order to allow them to be changed.
///
/// Exposed values of a controller are called *Controls*, which can be set/get by index.
/// Controller and controls can also provide extra information through their `Metadata`.
///
/// The crate *libfoxlive_derive* provides macros in order to implement this trait:
///
/// ```rust
/// #[object("optional_name")]
/// #[meta(description, "simple dsp")]
/// pub struct MyDSP {
///     #[control(F32(1.0,1.0,0.1), "param1"]
///     pub param1: f32,
///     #[control(U8(0,127,1)]
///     #[meta(widget="vslider")]
///     pub param2: u8,
/// }
/// ```
///
pub trait Controller {
    /// Return Controller's metadata
    fn get_metadata(&mut self) -> Metadatas {
        Metadatas::new()
    }

    /// Get value of a control
    fn get_control(&self, control: ControlIndex) -> Option<ControlValue> {
        None
    }

    /// Set value of a control
    fn set_control(&mut self, control: ControlIndex, value: ControlValue) -> Result<ControlValue, ()> {
        Err(())
    }

    /// Init a control mapper declaring all available controls
    fn map_controls(&self, mapper: &mut dyn ControlsMapper) {}
}


