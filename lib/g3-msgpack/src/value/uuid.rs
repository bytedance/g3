/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use rmpv::ValueRef;
use uuid::Uuid;

pub fn as_uuid(v: &ValueRef) -> anyhow::Result<Uuid> {
    match v {
        ValueRef::String(s) => {
            if let Some(s) = s.as_str() {
                Uuid::parse_str(s).map_err(|e| anyhow!("invalid encoded uuid string: {e}"))
            } else {
                Err(anyhow!("invalid utf-8 string"))
            }
        }
        ValueRef::Binary(b) => Uuid::from_slice(b).map_err(|e| anyhow!("invalid uuid bytes: {e}")),
        _ => Err(anyhow!(
            "msgpack value type for 'uuid' should be 'binary' or 'string'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmpv::Utf8StringRef;

    #[test]
    fn t_uuid() {
        let slice_v: [u8; 16] = [
            0x70, 0xa7, 0xc2, 0xbb, 0x47, 0x6f, 0x4d, 0x79, 0x8a, 0x38, 0xc7, 0xc6, 0xaf, 0xfb,
            0xfa, 0xf7,
        ];
        let tv = Uuid::from_slice(&slice_v).unwrap();

        let v = ValueRef::String(Utf8StringRef::from("70a7c2bb-476f-4d79-8a38-c7c6affbfaf7"));
        let pv = as_uuid(&v).unwrap();
        assert_eq!(pv, tv);

        let v = ValueRef::String(Utf8StringRef::from("70a7c2bb476f4d798a38c7c6affbfaf7"));
        let pv = as_uuid(&v).unwrap();
        assert_eq!(pv, tv);

        let v = ValueRef::String(Utf8StringRef::from("70a7c2bb476f4d798a38c7c6affbfaf"));
        assert!(as_uuid(&v).is_err());

        let v = ValueRef::Binary(&slice_v);
        let pv = as_uuid(&v).unwrap();
        assert_eq!(pv, tv);

        let v = ValueRef::F32(0.0);
        assert!(as_uuid(&v).is_err());
    }
}
