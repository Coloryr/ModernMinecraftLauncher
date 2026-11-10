use super::*;
use std::io::Cursor;

use crate::{NbtStream, NbtType};

#[test]
fn string_lengths_use_unsigned_bytes() {
    for len in [32767, 32768, 65535] {
        let value = "x".repeat(len);
        let mut compound = NbtCompound::new();
        compound
            .data
            .insert(value.clone(), NbtString::new(value.clone()).to_nbt());
        let mut bytes = Vec::new();
        compound.write(&mut bytes).unwrap();
        let mut decoded = NbtCompound::new();
        decoded.read(&mut Cursor::new(bytes)).unwrap();
        assert!(compound.eq(&decoded.to_nbt()));
    }
    for value in ["x".repeat(65536), "界".repeat(21846)] {
        assert!(
            NbtString::new(value.clone())
                .write(&mut Vec::new())
                .is_err()
        );
        let mut compound = NbtCompound::new();
        compound.data.insert(value, NbtByte::new(1).to_nbt());
        assert!(compound.write(&mut Vec::new()).is_err());
    }
}

#[test]
fn array_lengths_reject_negative_overflow_and_truncation() {
    for count in [-1i32, i32::MIN, i32::MAX, i32::MAX / 4 + 1, 1] {
        let bytes = count.to_be_bytes();
        assert!(
            NbtByteArray::default()
                .read(&mut Cursor::new(bytes))
                .is_err()
        );
        assert!(
            NbtIntArray::default()
                .read(&mut Cursor::new(bytes))
                .is_err()
        );
        assert!(
            NbtLongArray::default()
                .read(&mut Cursor::new(bytes))
                .is_err()
        );
    }
    let arrays = [
        NbtByteArray::new(vec![0, 255]).to_nbt(),
        NbtIntArray::new(vec![i32::MIN, i32::MAX]).to_nbt(),
        NbtLongArray::new(vec![i64::MIN, i64::MAX]).to_nbt(),
    ];
    for original in arrays {
        let mut bytes = Vec::new();
        original.write(&mut bytes).unwrap();
        let mut result = NbtType::get_nbt(original.get_num()).unwrap();
        result.read(&mut Cursor::new(bytes)).unwrap();
        assert!(original.eq(&result));
    }
}

