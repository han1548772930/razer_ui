//! THX V4 protobuf declarations recovered from current shipped PE bytes.
//!
//! Native descriptor RVA
//! 0xB61520, SHA-256 03dc9f18e6a984a3df19dcd2113adef733352ab1035ac599014ef96bb553876f.
//! This module performs no I/O. JSON conversion and service availability are
//! separate from the observed descriptor. Size/field/depth limits are Rust
//! boundary policy; they are not claims about the native client's limits.
use std::fmt;

#[cfg(test)]
#[path = "thx_v4_proto_tests.rs"]
mod tests;

pub const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_FIELDS: usize = 65_536;
const MAX_GROUP_DEPTH: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodecError {
    Truncated,
    InvalidVarint,
    InvalidTag(u64),
    InvalidWireType(u8),
    WrongWireType {
        field: u32,
        expected: u8,
        actual: u8,
    },
    InvalidUtf8,
    SizeLimit {
        limit: usize,
        actual: usize,
    },
    FieldLimit,
    GroupDepthLimit,
    MismatchedGroup,
    InvalidPackedFloat,
    PropertyIdMismatch,
    PropertyTypeMismatch,
    MissingPropertyValue,
    NotUnary,
    InvalidGrpcFlag(u8),
    CompressionUnsupported,
    MissingTrailers,
    InvalidGrpcStatus,
    GrpcStatus {
        code: u32,
        message: Option<String>,
    },
    HttpStatus(u16),
    InvalidContentType,
    UnaryFrameCount(usize),
}
impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "THX protobuf/gRPC: {self:?}")
    }
}
impl std::error::Error for CodecError {}
pub type CodecResult<T> = Result<T, CodecError>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PropertyType {
    #[default]
    None,
    Bool,
    Int,
    Float,
    String,
    StringArray,
    FloatArray,
    Count,
    Unknown(i32),
}
impl PropertyType {
    pub fn number(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Bool => 1,
            Self::Int => 2,
            Self::Float => 3,
            Self::String => 4,
            Self::StringArray => 5,
            Self::FloatArray => 6,
            Self::Count => 7,
            Self::Unknown(value) => value,
        }
    }
    pub fn from_number(value: i32) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Bool,
            2 => Self::Int,
            3 => Self::Float,
            4 => Self::String,
            5 => Self::StringArray,
            6 => Self::FloatArray,
            7 => Self::Count,
            value => Self::Unknown(value),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresetEvent {
    #[default]
    Added,
    Removed,
    Selected,
    Unknown(i32),
}
impl PresetEvent {
    pub fn number(self) -> i32 {
        match self {
            Self::Added => 0,
            Self::Removed => 1,
            Self::Selected => 2,
            Self::Unknown(n) => n,
        }
    }
    pub fn from_number(n: i32) -> Self {
        match n {
            0 => Self::Added,
            1 => Self::Removed,
            2 => Self::Selected,
            n => Self::Unknown(n),
        }
    }
}

/// All methods in the recovered service, including its two notification streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    GetEndpointList,
    GetPropertyList,
    GetPropertyDefinition,
    GetProperty,
    SetProperty,
    SubscribeToPropertyChanges,
    GetPresetList,
    SelectPreset,
    GetSelectedPreset,
    SavePreset,
    ResetPreset,
    AddPreset,
    RemovePreset,
    GetPresetDescription,
    SubscribeToPresetChanges,
    GetVersion,
}
impl Method {
    pub const ALL: [Self; 16] = [
        Self::GetEndpointList,
        Self::GetPropertyList,
        Self::GetPropertyDefinition,
        Self::GetProperty,
        Self::SetProperty,
        Self::SubscribeToPropertyChanges,
        Self::GetPresetList,
        Self::SelectPreset,
        Self::GetSelectedPreset,
        Self::SavePreset,
        Self::ResetPreset,
        Self::AddPreset,
        Self::RemovePreset,
        Self::GetPresetDescription,
        Self::SubscribeToPresetChanges,
        Self::GetVersion,
    ];
    pub fn path(self) -> &'static str {
        match self {
            Self::GetEndpointList => "/thx.proto.ThxAudioProperties/GetEndpointList",
            Self::GetPropertyList => "/thx.proto.ThxAudioProperties/GetPropertyList",
            Self::GetPropertyDefinition => "/thx.proto.ThxAudioProperties/GetPropertyDefinition",
            Self::GetProperty => "/thx.proto.ThxAudioProperties/GetProperty",
            Self::SetProperty => "/thx.proto.ThxAudioProperties/SetProperty",
            Self::SubscribeToPropertyChanges => {
                "/thx.proto.ThxAudioProperties/SubscribeToPropertyChanges"
            }
            Self::GetPresetList => "/thx.proto.ThxAudioProperties/GetPresetList",
            Self::SelectPreset => "/thx.proto.ThxAudioProperties/SelectPreset",
            Self::GetSelectedPreset => "/thx.proto.ThxAudioProperties/GetSelectedPreset",
            Self::SavePreset => "/thx.proto.ThxAudioProperties/SavePreset",
            Self::ResetPreset => "/thx.proto.ThxAudioProperties/ResetPreset",
            Self::AddPreset => "/thx.proto.ThxAudioProperties/AddPreset",
            Self::RemovePreset => "/thx.proto.ThxAudioProperties/RemovePreset",
            Self::GetPresetDescription => "/thx.proto.ThxAudioProperties/GetPresetDescription",
            Self::SubscribeToPresetChanges => {
                "/thx.proto.ThxAudioProperties/SubscribeToPresetChanges"
            }
            Self::GetVersion => "/thx.proto.ThxAudioProperties/GetVersion",
        }
    }
    pub fn from_path(path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.path() == path)
    }
    pub fn server_streaming(self) -> bool {
        matches!(
            self,
            Self::SubscribeToPropertyChanges | Self::SubscribeToPresetChanges
        )
    }
}

trait Message: Sized + Default {
    fn encode_body(&self) -> CodecResult<Vec<u8>>;
    fn merge_body(&mut self, bytes: &[u8]) -> CodecResult<()>;
    fn decode_body(bytes: &[u8]) -> CodecResult<Self> {
        let mut value = Self::default();
        value.merge_body(bytes)?;
        Ok(value)
    }
}
macro_rules! message {
    ($visibility:vis $name:ident { $($field:ident : $ty:ty = $number:literal => $kind:ident),* $(,)? }) => {
        #[derive(Clone, Debug, Default, PartialEq)]
        $visibility struct $name { $(pub $field: $ty),* }
        impl Message for $name {
            fn encode_body(&self) -> CodecResult<Vec<u8>> {
                let mut writer = Writer::default();
                $(writer.$kind($number, &self.$field)?;)*
                Ok(writer.bytes)
            }
            fn merge_body(&mut self, bytes: &[u8]) -> CodecResult<()> {
                decode_fields(bytes, |field| {
                    match field.number { $($number => field.$kind(&mut self.$field)?,)* _ => {} }
                    Ok(())
                })?;
                Ok(())
            }
        }
        impl $name {
            pub fn encode(&self) -> CodecResult<Vec<u8>> { self.encode_body() }
            pub fn decode(bytes: &[u8]) -> CodecResult<Self> { Self::decode_body(bytes) }
        }
    };
}
message!(pub Property {
    id: String = 1 => string,
    property_type: PropertyType = 2 => property_type,
    float_value: Option<f32> = 3 => optional_float,
    int_value: Option<i64> = 4 => optional_int64,
    bool_value: Option<bool> = 5 => optional_bool,
    string_value: Option<String> = 6 => optional_string,
    string_array_value: Vec<String> = 7 => strings,
    float_array_value: Vec<f32> = 8 => floats,
});
impl Property {
    pub fn validate_observation(
        &self,
        expected_id: &str,
        expected_type: PropertyType,
    ) -> CodecResult<()> {
        if self.id != expected_id {
            return Err(CodecError::PropertyIdMismatch);
        }
        if self.property_type != expected_type {
            return Err(CodecError::PropertyTypeMismatch);
        }
        Ok(())
    }
    pub fn observed_bool(&self, expected_id: &str) -> CodecResult<bool> {
        self.validate_observation(expected_id, PropertyType::Bool)?;
        self.bool_value.ok_or(CodecError::MissingPropertyValue)
    }
}
message!(pub BoolPropertyTraits { default_value: bool = 1 => boolean });
message!(pub FloatPropertyTraits {
    min_value: f32 = 1 => float, max_value: f32 = 2 => float,
    step: f32 = 3 => float, default_value: f32 = 4 => float,
});
message!(pub IntPropertyTraits {
    min_value: i32 = 1 => int32, max_value: i32 = 2 => int32, default_value: i32 = 4 => int32,
});
message!(pub StringPropertyTraits {
    default_value: String = 1 => string, allowed_values: Vec<String> = 2 => strings,
});
message!(pub StringArrayPropertyTraits {
    size: i32 = 1 => int32, default_value: Vec<String> = 2 => strings, allowed_values: Vec<String> = 3 => strings,
});
message!(pub FloatArrayPropertyTraits {
    min_value: f32 = 1 => float, max_value: f32 = 2 => float, step: f32 = 3 => float,
    size: i32 = 4 => int32, default_value: Vec<f32> = 5 => floats,
});
message!(pub PropertyDefinition {
    id: String = 1 => string, property_type: PropertyType = 2 => property_type,
    bool_traits: Option<BoolPropertyTraits> = 3 => message,
    float_traits: Option<FloatPropertyTraits> = 4 => message,
    int_traits: Option<IntPropertyTraits> = 5 => message,
    string_traits: Option<StringPropertyTraits> = 6 => message,
    string_array_traits: Option<StringArrayPropertyTraits> = 7 => message,
    float_array_traits: Option<FloatArrayPropertyTraits> = 8 => message,
});
message!(pub PresetChange {
    event: PresetEvent = 1 => preset_event, preset_group_id: String = 2 => string, preset_id: String = 3 => string,
});

message!(EndpointListBody { endpoint_type: String = 1 => string });
message!(EndpointBody { endpoint_id: Option<String> = 1 => optional_string });
message!(QueryBody { id: String = 1 => string, endpoint_id: Option<String> = 2 => optional_string });
message!(SetPropertyBody { property: Option<Property> = 1 => message, endpoint_id: Option<String> = 2 => optional_string });
message!(PresetBody {
    preset_group_id: String = 1 => string, preset_id: String = 2 => string, endpoint_id: Option<String> = 3 => optional_string,
});
message!(AddPresetBody {
    preset_group_id: String = 1 => string, source_preset_id: String = 2 => string,
    preset_id: String = 3 => string, endpoint_id: Option<String> = 4 => optional_string,
    description: Option<String> = 5 => optional_string,
});
message!(SelectedPresetBody { preset_group_id: String = 1 => string, preset_id: String = 2 => string });
message!(StringListBody { values: Vec<String> = 1 => strings });
message!(StringBody { value: String = 1 => string });

#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    GetEndpointList {
        endpoint_type: String,
    },
    GetPropertyList {
        endpoint_id: Option<String>,
    },
    GetPropertyDefinition {
        id: String,
        endpoint_id: Option<String>,
    },
    GetProperty {
        id: String,
        endpoint_id: Option<String>,
    },
    SetProperty {
        property: Option<Property>,
        endpoint_id: Option<String>,
    },
    SubscribeToPropertyChanges {
        endpoint_id: Option<String>,
    },
    GetPresetList {
        preset_group_id: String,
        endpoint_id: Option<String>,
    },
    SelectPreset {
        preset_group_id: String,
        preset_id: String,
        endpoint_id: Option<String>,
    },
    GetSelectedPreset {
        preset_group_id: String,
        endpoint_id: Option<String>,
    },
    SavePreset {
        preset_group_id: String,
        preset_id: String,
        endpoint_id: Option<String>,
    },
    ResetPreset {
        preset_group_id: String,
        preset_id: String,
        endpoint_id: Option<String>,
    },
    AddPreset {
        preset_group_id: String,
        source_preset_id: String,
        preset_id: String,
        endpoint_id: Option<String>,
        description: Option<String>,
    },
    RemovePreset {
        preset_group_id: String,
        preset_id: String,
        endpoint_id: Option<String>,
    },
    GetPresetDescription {
        preset_group_id: String,
        preset_id: String,
        endpoint_id: Option<String>,
    },
    SubscribeToPresetChanges {
        endpoint_id: Option<String>,
    },
    GetVersion,
}
impl Request {
    pub fn method(&self) -> Method {
        match self {
            Self::GetEndpointList { .. } => Method::GetEndpointList,
            Self::GetPropertyList { .. } => Method::GetPropertyList,
            Self::GetPropertyDefinition { .. } => Method::GetPropertyDefinition,
            Self::GetProperty { .. } => Method::GetProperty,
            Self::SetProperty { .. } => Method::SetProperty,
            Self::SubscribeToPropertyChanges { .. } => Method::SubscribeToPropertyChanges,
            Self::GetPresetList { .. } => Method::GetPresetList,
            Self::SelectPreset { .. } => Method::SelectPreset,
            Self::GetSelectedPreset { .. } => Method::GetSelectedPreset,
            Self::SavePreset { .. } => Method::SavePreset,
            Self::ResetPreset { .. } => Method::ResetPreset,
            Self::AddPreset { .. } => Method::AddPreset,
            Self::RemovePreset { .. } => Method::RemovePreset,
            Self::GetPresetDescription { .. } => Method::GetPresetDescription,
            Self::SubscribeToPresetChanges { .. } => Method::SubscribeToPresetChanges,
            Self::GetVersion => Method::GetVersion,
        }
    }
    pub fn encode(&self) -> CodecResult<Vec<u8>> {
        match self {
            Self::GetEndpointList { endpoint_type } => EndpointListBody {
                endpoint_type: endpoint_type.clone(),
            }
            .encode(),
            Self::GetPropertyList { endpoint_id }
            | Self::SubscribeToPropertyChanges { endpoint_id }
            | Self::SubscribeToPresetChanges { endpoint_id } => EndpointBody {
                endpoint_id: endpoint_id.clone(),
            }
            .encode(),
            Self::GetPropertyDefinition { id, endpoint_id }
            | Self::GetProperty { id, endpoint_id } => QueryBody {
                id: id.clone(),
                endpoint_id: endpoint_id.clone(),
            }
            .encode(),
            Self::SetProperty {
                property,
                endpoint_id,
            } => SetPropertyBody {
                property: property.clone(),
                endpoint_id: endpoint_id.clone(),
            }
            .encode(),
            Self::GetPresetList {
                preset_group_id,
                endpoint_id,
            }
            | Self::GetSelectedPreset {
                preset_group_id,
                endpoint_id,
            } => QueryBody {
                id: preset_group_id.clone(),
                endpoint_id: endpoint_id.clone(),
            }
            .encode(),
            Self::SelectPreset {
                preset_group_id,
                preset_id,
                endpoint_id,
            }
            | Self::SavePreset {
                preset_group_id,
                preset_id,
                endpoint_id,
            }
            | Self::ResetPreset {
                preset_group_id,
                preset_id,
                endpoint_id,
            }
            | Self::RemovePreset {
                preset_group_id,
                preset_id,
                endpoint_id,
            }
            | Self::GetPresetDescription {
                preset_group_id,
                preset_id,
                endpoint_id,
            } => PresetBody {
                preset_group_id: preset_group_id.clone(),
                preset_id: preset_id.clone(),
                endpoint_id: endpoint_id.clone(),
            }
            .encode(),
            Self::AddPreset {
                preset_group_id,
                source_preset_id,
                preset_id,
                endpoint_id,
                description,
            } => AddPresetBody {
                preset_group_id: preset_group_id.clone(),
                source_preset_id: source_preset_id.clone(),
                preset_id: preset_id.clone(),
                endpoint_id: endpoint_id.clone(),
                description: description.clone(),
            }
            .encode(),
            Self::GetVersion => Ok(Vec::new()),
        }
    }
    pub fn encode_framed(&self) -> CodecResult<Vec<u8>> {
        encode_grpc_frame(&self.encode()?)
    }
    pub fn decode(method: Method, bytes: &[u8]) -> CodecResult<Self> {
        Ok(match method {
            Method::GetEndpointList => Self::GetEndpointList {
                endpoint_type: EndpointListBody::decode(bytes)?.endpoint_type,
            },
            Method::GetPropertyList => Self::GetPropertyList {
                endpoint_id: EndpointBody::decode(bytes)?.endpoint_id,
            },
            Method::SubscribeToPropertyChanges => Self::SubscribeToPropertyChanges {
                endpoint_id: EndpointBody::decode(bytes)?.endpoint_id,
            },
            Method::SubscribeToPresetChanges => Self::SubscribeToPresetChanges {
                endpoint_id: EndpointBody::decode(bytes)?.endpoint_id,
            },
            Method::GetPropertyDefinition | Method::GetProperty => {
                let body = QueryBody::decode(bytes)?;
                if method == Method::GetProperty {
                    Self::GetProperty {
                        id: body.id,
                        endpoint_id: body.endpoint_id,
                    }
                } else {
                    Self::GetPropertyDefinition {
                        id: body.id,
                        endpoint_id: body.endpoint_id,
                    }
                }
            }
            Method::SetProperty => {
                let body = SetPropertyBody::decode(bytes)?;
                Self::SetProperty {
                    property: body.property,
                    endpoint_id: body.endpoint_id,
                }
            }
            Method::GetPresetList | Method::GetSelectedPreset => {
                let body = QueryBody::decode(bytes)?;
                if method == Method::GetPresetList {
                    Self::GetPresetList {
                        preset_group_id: body.id,
                        endpoint_id: body.endpoint_id,
                    }
                } else {
                    Self::GetSelectedPreset {
                        preset_group_id: body.id,
                        endpoint_id: body.endpoint_id,
                    }
                }
            }
            Method::SelectPreset
            | Method::SavePreset
            | Method::ResetPreset
            | Method::RemovePreset
            | Method::GetPresetDescription => {
                let b = PresetBody::decode(bytes)?;
                match method {
                    Method::SelectPreset => Self::SelectPreset {
                        preset_group_id: b.preset_group_id,
                        preset_id: b.preset_id,
                        endpoint_id: b.endpoint_id,
                    },
                    Method::SavePreset => Self::SavePreset {
                        preset_group_id: b.preset_group_id,
                        preset_id: b.preset_id,
                        endpoint_id: b.endpoint_id,
                    },
                    Method::ResetPreset => Self::ResetPreset {
                        preset_group_id: b.preset_group_id,
                        preset_id: b.preset_id,
                        endpoint_id: b.endpoint_id,
                    },
                    Method::RemovePreset => Self::RemovePreset {
                        preset_group_id: b.preset_group_id,
                        preset_id: b.preset_id,
                        endpoint_id: b.endpoint_id,
                    },
                    _ => Self::GetPresetDescription {
                        preset_group_id: b.preset_group_id,
                        preset_id: b.preset_id,
                        endpoint_id: b.endpoint_id,
                    },
                }
            }
            Method::AddPreset => {
                let b = AddPresetBody::decode(bytes)?;
                Self::AddPreset {
                    preset_group_id: b.preset_group_id,
                    source_preset_id: b.source_preset_id,
                    preset_id: b.preset_id,
                    endpoint_id: b.endpoint_id,
                    description: b.description,
                }
            }
            Method::GetVersion => {
                decode_fields(bytes, |_| Ok(()))?;
                Self::GetVersion
            }
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Response {
    GetEndpointList {
        endpoint_ids: Vec<String>,
    },
    GetPropertyList {
        properties: Vec<String>,
    },
    GetPropertyDefinition(PropertyDefinition),
    GetProperty(Property),
    SetProperty,
    SubscribeToPropertyChanges(Property),
    GetPresetList {
        preset_ids: Vec<String>,
    },
    SelectPreset,
    GetSelectedPreset {
        preset_group_id: String,
        preset_id: String,
    },
    SavePreset,
    ResetPreset,
    AddPreset {
        preset_ids: Vec<String>,
    },
    RemovePreset {
        preset_ids: Vec<String>,
    },
    GetPresetDescription {
        description: String,
    },
    SubscribeToPresetChanges(PresetChange),
    GetVersion {
        version: String,
    },
}
impl Response {
    pub fn decode(method: Method, bytes: &[u8]) -> CodecResult<Self> {
        Ok(match method {
            Method::GetEndpointList => Self::GetEndpointList {
                endpoint_ids: StringListBody::decode(bytes)?.values,
            },
            Method::GetPropertyList => Self::GetPropertyList {
                properties: StringListBody::decode(bytes)?.values,
            },
            Method::GetPropertyDefinition => {
                Self::GetPropertyDefinition(PropertyDefinition::decode(bytes)?)
            }
            Method::GetProperty => Self::GetProperty(Property::decode(bytes)?),
            Method::SubscribeToPropertyChanges => {
                Self::SubscribeToPropertyChanges(Property::decode(bytes)?)
            }
            Method::GetPresetList => Self::GetPresetList {
                preset_ids: StringListBody::decode(bytes)?.values,
            },
            Method::GetSelectedPreset => {
                let b = SelectedPresetBody::decode(bytes)?;
                Self::GetSelectedPreset {
                    preset_group_id: b.preset_group_id,
                    preset_id: b.preset_id,
                }
            }
            Method::AddPreset => Self::AddPreset {
                preset_ids: StringListBody::decode(bytes)?.values,
            },
            Method::RemovePreset => Self::RemovePreset {
                preset_ids: StringListBody::decode(bytes)?.values,
            },
            Method::GetPresetDescription => Self::GetPresetDescription {
                description: StringBody::decode(bytes)?.value,
            },
            Method::SubscribeToPresetChanges => {
                Self::SubscribeToPresetChanges(PresetChange::decode(bytes)?)
            }
            Method::GetVersion => Self::GetVersion {
                version: StringBody::decode(bytes)?.value,
            },
            Method::SetProperty
            | Method::SelectPreset
            | Method::SavePreset
            | Method::ResetPreset => {
                decode_fields(bytes, |_| Ok(()))?;
                match method {
                    Method::SetProperty => Self::SetProperty,
                    Method::SelectPreset => Self::SelectPreset,
                    Method::SavePreset => Self::SavePreset,
                    _ => Self::ResetPreset,
                }
            }
        })
    }
    pub fn encode(&self) -> CodecResult<Vec<u8>> {
        match self {
            Self::GetEndpointList { endpoint_ids } => StringListBody {
                values: endpoint_ids.clone(),
            }
            .encode(),
            Self::GetPropertyList { properties } => StringListBody {
                values: properties.clone(),
            }
            .encode(),
            Self::GetPropertyDefinition(value) => value.encode(),
            Self::GetProperty(value) | Self::SubscribeToPropertyChanges(value) => value.encode(),
            Self::GetPresetList { preset_ids }
            | Self::AddPreset { preset_ids }
            | Self::RemovePreset { preset_ids } => StringListBody {
                values: preset_ids.clone(),
            }
            .encode(),
            Self::GetSelectedPreset {
                preset_group_id,
                preset_id,
            } => SelectedPresetBody {
                preset_group_id: preset_group_id.clone(),
                preset_id: preset_id.clone(),
            }
            .encode(),
            Self::GetPresetDescription { description } => StringBody {
                value: description.clone(),
            }
            .encode(),
            Self::GetVersion { version } => StringBody {
                value: version.clone(),
            }
            .encode(),
            Self::SubscribeToPresetChanges(value) => value.encode(),
            Self::SetProperty | Self::SelectPreset | Self::SavePreset | Self::ResetPreset => {
                Ok(Vec::new())
            }
        }
    }
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
    fields: usize,
}
impl Writer {
    fn append(&mut self, bytes: &[u8]) -> CodecResult<()> {
        let actual = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(CodecError::SizeLimit {
                limit: MAX_MESSAGE_BYTES,
                actual: usize::MAX,
            })?;
        if actual > MAX_MESSAGE_BYTES {
            return Err(CodecError::SizeLimit {
                limit: MAX_MESSAGE_BYTES,
                actual,
            });
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn varint(&mut self, mut value: u64) -> CodecResult<()> {
        loop {
            let byte = (value & 127) as u8;
            value >>= 7;
            self.append(&[byte | if value == 0 { 0 } else { 128 }])?;
            if value == 0 {
                return Ok(());
            }
        }
    }
    fn key(&mut self, number: u32, wire: u8) -> CodecResult<()> {
        self.fields += 1;
        if self.fields > MAX_FIELDS {
            return Err(CodecError::FieldLimit);
        }
        self.varint(((number as u64) << 3) | wire as u64)
    }
    fn blob(&mut self, number: u32, bytes: &[u8]) -> CodecResult<()> {
        self.key(number, 2)?;
        self.varint(bytes.len() as u64)?;
        self.append(bytes)
    }
    fn string(&mut self, number: u32, value: &str) -> CodecResult<()> {
        if !value.is_empty() {
            self.blob(number, value.as_bytes())?;
        }
        Ok(())
    }
    fn optional_string(&mut self, n: u32, value: &Option<String>) -> CodecResult<()> {
        if let Some(value) = value {
            self.blob(n, value.as_bytes())?;
        }
        Ok(())
    }
    fn strings(&mut self, n: u32, value: &[String]) -> CodecResult<()> {
        for value in value {
            self.blob(n, value.as_bytes())?;
        }
        Ok(())
    }
    fn float(&mut self, n: u32, value: &f32) -> CodecResult<()> {
        if value.to_bits() != 0 {
            self.key(n, 5)?;
            self.append(&value.to_le_bytes())?;
        }
        Ok(())
    }
    fn optional_float(&mut self, n: u32, value: &Option<f32>) -> CodecResult<()> {
        if let Some(value) = value {
            self.key(n, 5)?;
            self.append(&value.to_le_bytes())?;
        }
        Ok(())
    }
    fn floats(&mut self, n: u32, values: &[f32]) -> CodecResult<()> {
        if values.is_empty() {
            return Ok(());
        }
        if values.len() > MAX_FIELDS {
            return Err(CodecError::FieldLimit);
        }
        let actual = values.len().checked_mul(4).ok_or(CodecError::SizeLimit {
            limit: MAX_MESSAGE_BYTES,
            actual: usize::MAX,
        })?;
        if actual > MAX_MESSAGE_BYTES {
            return Err(CodecError::SizeLimit {
                limit: MAX_MESSAGE_BYTES,
                actual,
            });
        }
        self.key(n, 2)?;
        self.varint(actual as u64)?;
        for value in values {
            self.append(&value.to_le_bytes())?;
        }
        Ok(())
    }
    fn int32(&mut self, n: u32, value: &i32) -> CodecResult<()> {
        if *value != 0 {
            self.key(n, 0)?;
            self.varint(*value as i64 as u64)?;
        }
        Ok(())
    }
    fn optional_int64(&mut self, n: u32, value: &Option<i64>) -> CodecResult<()> {
        if let Some(value) = value {
            self.key(n, 0)?;
            self.varint(*value as u64)?;
        }
        Ok(())
    }
    fn boolean(&mut self, n: u32, value: &bool) -> CodecResult<()> {
        if *value {
            self.key(n, 0)?;
            self.varint(1)?;
        }
        Ok(())
    }
    fn optional_bool(&mut self, n: u32, value: &Option<bool>) -> CodecResult<()> {
        if let Some(value) = value {
            self.key(n, 0)?;
            self.varint(u64::from(*value))?;
        }
        Ok(())
    }
    fn property_type(&mut self, n: u32, value: &PropertyType) -> CodecResult<()> {
        self.int32(n, &value.number())
    }
    fn preset_event(&mut self, n: u32, value: &PresetEvent) -> CodecResult<()> {
        self.int32(n, &value.number())
    }
    fn message<M: Message>(&mut self, n: u32, value: &Option<M>) -> CodecResult<()> {
        if let Some(value) = value {
            self.blob(n, &value.encode_body()?)?;
        }
        Ok(())
    }
}

enum FieldValue<'a> {
    Varint(u64),
    Fixed32(u32),
    Bytes(&'a [u8]),
    Ignored,
}
struct Field<'a> {
    number: u32,
    wire: u8,
    value: FieldValue<'a>,
}
impl Field<'_> {
    fn wrong(&self, expected: u8) -> CodecError {
        CodecError::WrongWireType {
            field: self.number,
            expected,
            actual: self.wire,
        }
    }
    fn number_value(&self) -> CodecResult<u64> {
        match self.value {
            FieldValue::Varint(n) => Ok(n),
            _ => Err(self.wrong(0)),
        }
    }
    fn float_value(&self) -> CodecResult<f32> {
        match self.value {
            FieldValue::Fixed32(n) => Ok(f32::from_bits(n)),
            _ => Err(self.wrong(5)),
        }
    }
    fn bytes(&self) -> CodecResult<&[u8]> {
        match self.value {
            FieldValue::Bytes(b) => Ok(b),
            _ => Err(self.wrong(2)),
        }
    }
    fn string(&self, value: &mut String) -> CodecResult<()> {
        *value = std::str::from_utf8(self.bytes()?)
            .map_err(|_| CodecError::InvalidUtf8)?
            .to_owned();
        Ok(())
    }
    fn optional_string(&self, value: &mut Option<String>) -> CodecResult<()> {
        let mut string = String::new();
        self.string(&mut string)?;
        *value = Some(string);
        Ok(())
    }
    fn strings(&self, values: &mut Vec<String>) -> CodecResult<()> {
        if values.len() >= MAX_FIELDS {
            return Err(CodecError::FieldLimit);
        }
        let mut string = String::new();
        self.string(&mut string)?;
        values.push(string);
        Ok(())
    }
    fn float(&self, value: &mut f32) -> CodecResult<()> {
        *value = self.float_value()?;
        Ok(())
    }
    fn optional_float(&self, value: &mut Option<f32>) -> CodecResult<()> {
        *value = Some(self.float_value()?);
        Ok(())
    }
    fn floats(&self, values: &mut Vec<f32>) -> CodecResult<()> {
        if self.wire == 5 {
            if values.len() >= MAX_FIELDS {
                return Err(CodecError::FieldLimit);
            }
            values.push(self.float_value()?);
        } else {
            let bytes = self.bytes()?;
            if bytes.len() % 4 != 0 {
                return Err(CodecError::InvalidPackedFloat);
            }
            if values
                .len()
                .checked_add(bytes.len() / 4)
                .is_none_or(|n| n > MAX_FIELDS)
            {
                return Err(CodecError::FieldLimit);
            }
            values.extend(
                bytes
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            );
        }
        Ok(())
    }
    fn int32(&self, value: &mut i32) -> CodecResult<()> {
        *value = self.number_value()? as i32;
        Ok(())
    }
    fn optional_int64(&self, value: &mut Option<i64>) -> CodecResult<()> {
        *value = Some(self.number_value()? as i64);
        Ok(())
    }
    fn boolean(&self, value: &mut bool) -> CodecResult<()> {
        *value = self.number_value()? != 0;
        Ok(())
    }
    fn optional_bool(&self, value: &mut Option<bool>) -> CodecResult<()> {
        *value = Some(self.number_value()? != 0);
        Ok(())
    }
    fn property_type(&self, value: &mut PropertyType) -> CodecResult<()> {
        *value = PropertyType::from_number(self.number_value()? as i32);
        Ok(())
    }
    fn preset_event(&self, value: &mut PresetEvent) -> CodecResult<()> {
        *value = PresetEvent::from_number(self.number_value()? as i32);
        Ok(())
    }
    fn message<M: Message>(&self, value: &mut Option<M>) -> CodecResult<()> {
        value
            .get_or_insert_with(M::default)
            .merge_body(self.bytes()?)
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    fields: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> CodecResult<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(CodecError::Truncated)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(CodecError::Truncated)?;
        self.position = end;
        Ok(bytes)
    }
    fn varint(&mut self) -> CodecResult<u64> {
        let mut value = 0u64;
        for index in 0..10 {
            let byte = self.take(1)?[0];
            if index == 9 && byte > 1 {
                return Err(CodecError::InvalidVarint);
            }
            value |= u64::from(byte & 127) << (index * 7);
            if byte & 128 == 0 {
                return Ok(value);
            }
        }
        Err(CodecError::InvalidVarint)
    }
    fn tag(&mut self) -> CodecResult<(u32, u8)> {
        self.fields += 1;
        if self.fields > MAX_FIELDS {
            return Err(CodecError::FieldLimit);
        }
        let key = self.varint()?;
        if key >> 3 == 0 || key >> 3 > 0x1fff_ffff {
            return Err(CodecError::InvalidTag(key));
        }
        Ok(((key >> 3) as u32, (key & 7) as u8))
    }
    fn value(&mut self, number: u32, wire: u8, depth: usize) -> CodecResult<FieldValue<'a>> {
        Ok(match wire {
            0 => FieldValue::Varint(self.varint()?),
            1 => {
                self.take(8)?;
                FieldValue::Ignored
            }
            2 => {
                let count = usize::try_from(self.varint()?).map_err(|_| CodecError::Truncated)?;
                FieldValue::Bytes(self.take(count)?)
            }
            3 => {
                if depth >= MAX_GROUP_DEPTH {
                    return Err(CodecError::GroupDepthLimit);
                }
                loop {
                    let (child, kind) = self.tag()?;
                    if kind == 4 {
                        if child != number {
                            return Err(CodecError::MismatchedGroup);
                        }
                        break;
                    }
                    self.value(child, kind, depth + 1)?;
                }
                FieldValue::Ignored
            }
            4 => return Err(CodecError::MismatchedGroup),
            5 => {
                let b = self.take(4)?;
                FieldValue::Fixed32(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            }
            kind => return Err(CodecError::InvalidWireType(kind)),
        })
    }
}
fn decode_fields(
    bytes: &[u8],
    mut handle: impl FnMut(Field<'_>) -> CodecResult<()>,
) -> CodecResult<()> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(CodecError::SizeLimit {
            limit: MAX_MESSAGE_BYTES,
            actual: bytes.len(),
        });
    }
    let mut reader = Reader {
        bytes,
        position: 0,
        fields: 0,
    };
    while reader.position < bytes.len() {
        let (number, wire) = reader.tag()?;
        let value = reader.value(number, wire, 0)?;
        handle(Field {
            number,
            wire,
            value,
        })?;
    }
    Ok(())
}

pub fn encode_grpc_frame(body: &[u8]) -> CodecResult<Vec<u8>> {
    if body.len() > MAX_MESSAGE_BYTES {
        return Err(CodecError::SizeLimit {
            limit: MAX_MESSAGE_BYTES,
            actual: body.len(),
        });
    }
    let mut frame = Vec::with_capacity(body.len() + 5);
    frame.push(0);
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(body);
    Ok(frame)
}
/// Feed arbitrary HTTP/2 DATA fragments. Outputs are complete uncompressed
/// protobuf bodies. A zero-length body is a real message, including Empty RPCs.
#[derive(Default)]
pub struct GrpcFrameDecoder {
    header: [u8; 5],
    header_len: usize,
    expected: Option<usize>,
    pending: Vec<u8>,
    failed: bool,
}
impl GrpcFrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, bytes: &[u8]) -> CodecResult<Vec<Vec<u8>>> {
        if self.failed {
            return Err(CodecError::Truncated);
        }
        // Bound returned-frame allocations as well as the unfinished frame.
        // The caller can split a larger buffer into smaller DATA chunks.
        if bytes.len() > MAX_MESSAGE_BYTES * 2 + 10 {
            self.failed = true;
            return Err(CodecError::SizeLimit {
                limit: MAX_MESSAGE_BYTES * 2 + 10,
                actual: bytes.len(),
            });
        }
        let result = self.push_inner(bytes);
        if result.is_err() {
            self.failed = true;
            self.pending.clear();
        }
        result
    }
    fn push_inner(&mut self, mut bytes: &[u8]) -> CodecResult<Vec<Vec<u8>>> {
        let mut complete = Vec::new();
        while !bytes.is_empty() {
            if self.expected.is_none() {
                let count = (5 - self.header_len).min(bytes.len());
                self.header[self.header_len..self.header_len + count]
                    .copy_from_slice(&bytes[..count]);
                self.header_len += count;
                bytes = &bytes[count..];
                if self.header_len != 5 {
                    continue;
                }
                match self.header[0] {
                    0 => {}
                    1 => return Err(CodecError::CompressionUnsupported),
                    flag => return Err(CodecError::InvalidGrpcFlag(flag)),
                }
                let length = u32::from_be_bytes([
                    self.header[1],
                    self.header[2],
                    self.header[3],
                    self.header[4],
                ]) as usize;
                if length > MAX_MESSAGE_BYTES {
                    return Err(CodecError::SizeLimit {
                        limit: MAX_MESSAGE_BYTES,
                        actual: length,
                    });
                }
                self.expected = Some(length);
                if length == 0 {
                    complete.push(Vec::new());
                    self.expected = None;
                    self.header_len = 0;
                }
            }
            if let Some(length) = self.expected {
                let count = (length - self.pending.len()).min(bytes.len());
                self.pending.extend_from_slice(&bytes[..count]);
                bytes = &bytes[count..];
                if self.pending.len() == length {
                    complete.push(std::mem::take(&mut self.pending));
                    self.expected = None;
                    self.header_len = 0;
                }
            }
            if complete.len() > MAX_FIELDS {
                return Err(CodecError::FieldLimit);
            }
        }
        Ok(complete)
    }
    pub fn finish(&self) -> CodecResult<()> {
        if self.failed || self.header_len != 0 || self.expected.is_some() {
            Err(CodecError::Truncated)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrpcTrailers {
    pub status: String,
    pub message: Option<String>,
}
impl GrpcTrailers {
    pub fn verify_success(&self) -> CodecResult<()> {
        if self.status.is_empty() || !self.status.bytes().all(|b| b.is_ascii_digit()) {
            return Err(CodecError::InvalidGrpcStatus);
        }
        let code: u32 = self
            .status
            .parse()
            .map_err(|_| CodecError::InvalidGrpcStatus)?;
        if code != 0 {
            return Err(CodecError::GrpcStatus {
                code,
                message: self.message.clone(),
            });
        }
        Ok(())
    }
}
/// An incremental unary collector which enforces both frame count and final
/// framing completion before returning a typed response.
#[derive(Default)]
pub struct UnaryGrpcResponse {
    decoder: GrpcFrameDecoder,
    frames: Vec<Vec<u8>>,
    error: Option<CodecError>,
}
impl UnaryGrpcResponse {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, bytes: &[u8]) -> CodecResult<()> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let result = self.decoder.push(bytes).and_then(|frames| {
            self.frames.extend(frames);
            if self.frames.len() > 1 {
                Err(CodecError::UnaryFrameCount(self.frames.len()))
            } else {
                Ok(())
            }
        });
        if let Err(error) = &result {
            self.error = Some(error.clone());
            self.frames.clear();
        }
        result
    }
    /// Call only after HTTP/2 END_STREAM; status must come from the received
    /// final trailers. No status is inferred from a protobuf body or HTTP 200.
    pub fn finish(
        self,
        method: Method,
        http_status: u16,
        content_type: Option<&str>,
        trailers: Option<&GrpcTrailers>,
    ) -> CodecResult<Response> {
        if let Some(error) = self.error {
            return Err(error);
        }
        self.decoder.finish()?;
        complete_unary(method, http_status, content_type, &self.frames, trailers)
    }
}
/// Complete only after the caller has reached END_STREAM and collected final
/// trailers. A transport error, partial body, absent status or nonzero status
/// cannot be turned into an Empty success. DATA framing remains independently
/// checked by `GrpcFrameDecoder::finish` before calling this function.
pub fn complete_unary(
    method: Method,
    http_status: u16,
    content_type: Option<&str>,
    frames: &[Vec<u8>],
    trailers: Option<&GrpcTrailers>,
) -> CodecResult<Response> {
    if method.server_streaming() {
        return Err(CodecError::NotUnary);
    }
    if http_status != 200 {
        return Err(CodecError::HttpStatus(http_status));
    }
    let content_type = content_type
        .ok_or(CodecError::InvalidContentType)?
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    if content_type != "application/grpc" && content_type != "application/grpc+proto" {
        return Err(CodecError::InvalidContentType);
    }
    trailers
        .ok_or(CodecError::MissingTrailers)?
        .verify_success()?;
    if frames.len() != 1 {
        return Err(CodecError::UnaryFrameCount(frames.len()));
    }
    Response::decode(method, &frames[0])
}
