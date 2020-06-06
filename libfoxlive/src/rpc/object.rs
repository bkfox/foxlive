//! Reflexive object that is used to integrate with user interfaces.
//!
use super::value::*;


/// Metadata as (key, value)
pub type Metadata = (&'static str, &'static str);

/// List of metadatas for controller and its controls
pub type Metadatas = Vec<Metadata>;

/// ObjectIndex to an exposed member
pub type ObjectIndex = u32;


pub enum ObjectEvent {
    /// Value changed
    Value(ObjectIndex, Value),
    /// A new child has been added to object
    ChildAppend(ObjectIndex),
    /// A child has been removed from object (can also be detected by closed broadcast)
    ChildRemoved(ObjectIndex),
}


/// Object's informations.
pub struct ObjectMeta {
    /// Name for humans
    pub name: String,
    /// Object metadata
    pub metadatas: Metadatas,
}

impl ObjectMeta {
    pub fn new<N: Into<String>>(name: N, metadatas: Option<Metadatas>) -> Self {
        Self {
            name: name.into(),
            metadatas: metadatas.or(Some(Metadatas::new())).unwrap(),
        }
    }
}


/// An object is a structure that can be manipulated by user interfaces. Its members
/// are mapped to the UI using an `ObjectMapper` visitor which is used to declare
/// object's metadata and values.
///
/// There is no support for parenting because this trait aims for the basic, providing field
/// info in order to automate interface building.
///
/// See `libfoxlive_derive` for procedural macros.
pub trait Object {
    /// Return Object metadata
    fn object_meta(&self) -> ObjectMeta;

    /// Get value by index
    fn get_value(&self, _index: ObjectIndex) -> Option<Value> {
        None
    }

    /// Set a value by index. Return `Ok` if it succeed, otherwise `Err`.
    fn set_value(&mut self, _index: ObjectIndex, _value: Value) -> Result<Value, ()> {
        Err(())
    }

    /// Visit object using the provided mapper.
    fn map_object(&self, _mapper: &mut dyn ObjectMapper) {}
}


/// Field information used for mapping
pub struct FieldInfo {
    pub index: ObjectIndex,
    pub value_type: ValueType,
    pub default: Option<Value>,
    pub range: Option<Range>,
    pub metadatas: Metadatas,
}


/// Trait providing interface to map object
pub trait ObjectMapper{
    /// Declare a member
    fn declare(&mut self, field_info: FieldInfo);
}

