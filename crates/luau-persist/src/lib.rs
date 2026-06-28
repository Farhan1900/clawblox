//! Luau VM persistence primitives.
//!
//! This crate is intentionally independent of any host engine. Host concepts
//! are represented through explicit adapters at higher layers; unsupported VM
//! state is reported as an error rather than reconstructed heuristically.

mod raw;

pub mod bridge;

pub use raw::{
    dump_stack_value, dump_stack_value_with_options, dump_stack_value_with_permanents,
    load_stack_value, load_stack_value_with_options, load_stack_value_with_permanents, DumpOptions,
    LoadOptions, PersistError, UserDataAdapter, UserDataPayload,
};

pub const FORMAT_MAGIC: &[u8; 8] = b"LUPST\0\0\x01";
pub const FORMAT_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Tag {
    Nil = 0x00,
    BooleanFalse = 0x01,
    BooleanTrue = 0x02,
    Number = 0x03,
    String = 0x04,
    Buffer = 0x05,
    Vector = 0x06,
    Table = 0x10,
    Reference = 0x11,
    Permanent = 0x12,
    Function = 0x20,
    Proto = 0x21,
    Upvalue = 0x22,
    Thread = 0x23,
    UserData = 0x30,
}

impl Tag {
    fn from_byte(byte: u8) -> Result<Self, PersistError> {
        match byte {
            0x00 => Ok(Self::Nil),
            0x01 => Ok(Self::BooleanFalse),
            0x02 => Ok(Self::BooleanTrue),
            0x03 => Ok(Self::Number),
            0x04 => Ok(Self::String),
            0x05 => Ok(Self::Buffer),
            0x06 => Ok(Self::Vector),
            0x10 => Ok(Self::Table),
            0x11 => Ok(Self::Reference),
            0x12 => Ok(Self::Permanent),
            0x20 => Ok(Self::Function),
            0x21 => Ok(Self::Proto),
            0x22 => Ok(Self::Upvalue),
            0x23 => Ok(Self::Thread),
            0x30 => Ok(Self::UserData),
            other => Err(PersistError::InvalidTag(other)),
        }
    }
}

fn write_header(out: &mut Vec<u8>) {
    out.extend_from_slice(FORMAT_MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
}

fn read_header(input: &[u8]) -> Result<&[u8], PersistError> {
    let header_len = FORMAT_MAGIC.len() + 2;
    if input.len() < header_len {
        return Err(PersistError::UnexpectedEof);
    }
    let (magic, rest) = input.split_at(FORMAT_MAGIC.len());
    if magic != FORMAT_MAGIC {
        return Err(PersistError::InvalidMagic);
    }
    let (version, rest) = rest.split_at(2);
    let version = u16::from_le_bytes([version[0], version[1]]);
    if version != FORMAT_VERSION {
        return Err(PersistError::UnsupportedVersion(version));
    }
    Ok(rest)
}
