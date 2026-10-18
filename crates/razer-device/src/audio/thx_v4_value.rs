//! THX V4 native JSON/Property conversion.
//!
//! The native converters are registered at RVAs 0x0FCE00 / 0x0FCF70.
//! These native compatibility conversions are not property observation checks:
//! acknowledged Bool reads must use `Property::observed_bool` first.
use crate::thx_v4::{ThxPropertyRequest, ThxPropertyType};
use crate::thx_v4_proto::{CodecError, Property, PropertyType};
use serde_json::{Map, Number, Value};
use std::fmt;

#[cfg(test)]
#[path = "thx_v4_value_tests.rs"]
mod tests;

pub const MAX_ARRAY_ELEMENTS: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueError {
    ExpectedObject,
    MissingField(&'static str),
    InvalidField(&'static str),
    UnknownType(String),
    UnsupportedType(i32),
    ExpectedValue {
        expected: &'static str,
        element: Option<usize>,
    },
    NoValue,
    ArrayLimit,
    Codec(CodecError),
}
impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "THX property conversion: {self:?}")
    }
}
impl std::error::Error for ValueError {}
pub type ValueResult<T> = Result<T, ValueError>;

fn type_from_name(name: &str) -> ValueResult<PropertyType> {
    Ok(match name {
        "None" => PropertyType::None,
        "Bool" => PropertyType::Bool,
        "Int" => PropertyType::Int,
        "Float" => PropertyType::Float,
        "String" => PropertyType::String,
        "StringArray" => PropertyType::StringArray,
        "FloatArray" => PropertyType::FloatArray,
        _ => return Err(ValueError::UnknownType(name.into())),
    })
}

/// Convert the source wrapper's typed request without re-inferring its type.
/// None intentionally ignores the supplied value, as native lambda_9 does.
pub fn property_from_request(request: &ThxPropertyRequest) -> ValueResult<Property> {
    let property_type = match &request.property_type {
        ThxPropertyType::Bool => PropertyType::Bool,
        ThxPropertyType::Int => PropertyType::Int,
        ThxPropertyType::Float => PropertyType::Float,
        ThxPropertyType::String => PropertyType::String,
        ThxPropertyType::StringArray => PropertyType::StringArray,
        ThxPropertyType::FloatArray => PropertyType::FloatArray,
        ThxPropertyType::None => PropertyType::None,
        ThxPropertyType::Explicit(name) => type_from_name(name)?,
    };
    property_from_parts(&request.id, property_type, Some(&request.value))
}

/// Static propertyFromJson boundary. Missing id/type are native rejection paths.
/// For value-bearing types a missing value is an explicit Rust error; the native
/// const object lookup has no prior presence check and dereferences its result.
pub fn property_from_json(json: &Value) -> ValueResult<Property> {
    let object = json.as_object().ok_or(ValueError::ExpectedObject)?;
    let id = object
        .get("id")
        .ok_or(ValueError::MissingField("id"))?
        .as_str()
        .ok_or(ValueError::InvalidField("id"))?;
    let name = object
        .get("type")
        .ok_or(ValueError::MissingField("type"))?
        .as_str()
        .ok_or(ValueError::InvalidField("type"))?;
    property_from_parts(id, type_from_name(name)?, object.get("value"))
}

fn property_from_parts(
    id: &str,
    property_type: PropertyType,
    value: Option<&Value>,
) -> ValueResult<Property> {
    let mut property = Property {
        id: id.into(),
        property_type,
        ..Property::default()
    };
    if property_type == PropertyType::None {
        property.encode().map_err(ValueError::Codec)?;
        return Ok(property);
    }
    let value = value.ok_or(ValueError::MissingField("value"))?;
    match property_type {
        PropertyType::Bool => {
            property.bool_value = Some(value.as_bool().ok_or_else(|| expected("boolean", None))?)
        }
        PropertyType::Int => property.int_value = Some(i64::from(native_int32(value, None)?)),
        PropertyType::Float => property.float_value = Some(native_float(value, None)?),
        PropertyType::String => {
            property.string_value = Some(
                value
                    .as_str()
                    .ok_or_else(|| expected("string", None))?
                    .into(),
            )
        }
        PropertyType::StringArray => {
            for (index, element) in native_values(value)?.into_iter().enumerate() {
                property.string_array_value.push(
                    element
                        .as_str()
                        .ok_or_else(|| expected("string", Some(index)))?
                        .into(),
                );
            }
        }
        PropertyType::FloatArray => {
            for (index, element) in native_values(value)?.into_iter().enumerate() {
                property
                    .float_array_value
                    .push(native_float(element, Some(index))?);
            }
        }
        other => return Err(ValueError::UnsupportedType(other.number())),
    }
    // Apply the shared Rust codec's resource boundary before a transport sees it.
    property.encode().map_err(ValueError::Codec)?;
    Ok(property)
}
fn expected(expected: &'static str, element: Option<usize>) -> ValueError {
    ValueError::ExpectedValue { expected, element }
}

fn native_int32(value: &Value, element: Option<usize>) -> ValueResult<i32> {
    match value {
        Value::Bool(value) => Ok(i32::from(*value)),
        Value::Number(number) if number.is_f64() => {
            // Native CVTTSD2SI writes 0x80000000 for an invalid/out-of-range
            // conversion with masked SSE exceptions. Rust's `as` saturates,
            // so spell out the observed instruction's indefinite result.
            let truncated = number.as_f64().unwrap().trunc();
            Ok(
                if truncated.is_finite() && truncated >= i32::MIN as f64 && truncated < 2147483648.0
                {
                    truncated as i32
                } else {
                    i32::MIN
                },
            )
        }
        Value::Number(number) => {
            let low = if let Some(unsigned) = number.as_u64() {
                unsigned as u32
            } else {
                number.as_i64().ok_or_else(|| expected("number", element))? as u32
            };
            Ok(low as i32)
        }
        _ => Err(expected("number", element)),
    }
}
fn native_float(value: &Value, element: Option<usize>) -> ValueResult<f32> {
    match value {
        Value::Bool(value) => Ok(u8::from(*value) as f32),
        Value::Number(number) if number.is_f64() => Ok(number.as_f64().unwrap() as f32),
        Value::Number(number) => {
            if let Some(unsigned) = number.as_u64() {
                // Exact unsigned high-bit path observed at RVA 0x1A54C3.
                Ok(if unsigned <= i64::MAX as u64 {
                    unsigned as i64 as f32
                } else {
                    let half = ((unsigned >> 1) | (unsigned & 1)) as i64 as f32;
                    half + half
                })
            } else {
                Ok(number.as_i64().ok_or_else(|| expected("number", element))? as f32)
            }
        }
        _ => Err(expected("number", element)),
    }
}

fn native_values(value: &Value) -> ValueResult<Vec<&Value>> {
    // Native JSON begin/end iterators iterate object values in std::map order,
    // array elements in order, primitive values once, and null zero times.
    let length = match value {
        Value::Array(values) => values.len(),
        Value::Object(values) => values.len(),
        Value::Null => 0,
        _ => 1,
    };
    if length > MAX_ARRAY_ELEMENTS {
        return Err(ValueError::ArrayLimit);
    }
    Ok(match value {
        Value::Array(values) => values.iter().collect(),
        Value::Object(values) => {
            let mut ordered = values.iter().collect::<Vec<_>>();
            ordered.sort_unstable_by(|(left, _), (right, _)| left.as_bytes().cmp(right.as_bytes()));
            ordered.into_iter().map(|(_, value)| value).collect()
        }
        Value::Null => vec![],
        value => vec![value],
    })
}

/// GetProperty's value projection. Native scalar getters use default storage
/// even when optional presence bits are unset; None has no value key and yields
/// NoValue. This compatibility operation never acknowledges a read/write.
pub fn value_from_property(property: &Property) -> ValueResult<Value> {
    property.encode().map_err(ValueError::Codec)?;
    Ok(match property.property_type {
        PropertyType::None => return Err(ValueError::NoValue),
        PropertyType::Bool => Value::Bool(property.bool_value.unwrap_or(false)),
        PropertyType::Int => Value::Number(property.int_value.unwrap_or(0).into()),
        PropertyType::Float => json_float(property.float_value.unwrap_or(0.0)),
        PropertyType::String => Value::String(property.string_value.clone().unwrap_or_default()),
        PropertyType::StringArray => Value::Array(
            property
                .string_array_value
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
        PropertyType::FloatArray => Value::Array(
            property
                .float_array_value
                .iter()
                .copied()
                .map(json_float)
                .collect(),
        ),
        other => return Err(ValueError::UnsupportedType(other.number())),
    })
}
fn json_float(value: f32) -> Value {
    // GetProperty dumps the native JSON value; its float serializer emits null
    // for NaN and infinity. Finite f32 is widened to JSON's double storage.
    Number::from_f64(f64::from(value))
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

/// Full propertyToJson object; None retains id/type but omits value.
pub fn json_from_property(property: &Property) -> ValueResult<Value> {
    let name = match property.property_type {
        PropertyType::None => "None",
        PropertyType::Bool => "Bool",
        PropertyType::Int => "Int",
        PropertyType::Float => "Float",
        PropertyType::String => "String",
        PropertyType::StringArray => "StringArray",
        PropertyType::FloatArray => "FloatArray",
        other => return Err(ValueError::UnsupportedType(other.number())),
    };
    let mut object = Map::new();
    object.insert("id".into(), Value::String(property.id.clone()));
    object.insert("type".into(), Value::String(name.into()));
    match value_from_property(property) {
        Ok(value) => {
            object.insert("value".into(), value);
        }
        Err(ValueError::NoValue) => {}
        Err(error) => return Err(error),
    }
    Ok(Value::Object(object))
}
