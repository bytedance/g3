/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use tokio::io::{AsyncRead, AsyncReadExt};

use super::HeaderTransportResponse;
use crate::target::thrift::tcp::ThriftTcpResponseError;

#[derive(Default)]
pub(crate) struct KitexTTHeaderReader {}

const FIXED_HEADER_SIZE: usize = 14;

impl KitexTTHeaderReader {
    pub(crate) async fn read<'a, R>(
        &mut self,
        reader: &mut R,
        buf: &'a mut Vec<u8>,
    ) -> Result<HeaderTransportResponse<'a>, ThriftTcpResponseError>
    where
        R: AsyncRead + Unpin,
    {
        let start_offset = buf.len();

        let fixed_end_offset = start_offset + FIXED_HEADER_SIZE;
        buf.resize(fixed_end_offset, 0);
        let nr = reader
            .read_exact(&mut buf[start_offset..fixed_end_offset])
            .await
            .map_err(ThriftTcpResponseError::ReadFailed)?;
        if nr != FIXED_HEADER_SIZE {
            return Err(ThriftTcpResponseError::NoEnoughDataRead);
        }
        let fixed_header = &buf[start_offset..fixed_end_offset];
        let var_header_size = (((fixed_header[12] as usize) << 8) + fixed_header[13] as usize) * 4;
        // let var_header = &buf[fixed_end_offset..fixed_end_offset + var_header_size];
        // TODO parse var header

        let length = u32::from_be_bytes([
            fixed_header[0],
            fixed_header[1],
            fixed_header[2],
            fixed_header[3],
        ]) as usize;
        let seq_id = i32::from_be_bytes([
            fixed_header[8],
            fixed_header[9],
            fixed_header[10],
            fixed_header[11],
        ]);

        let end_offset = start_offset + 4 + length;
        buf.resize(end_offset, 0);
        let nr = reader
            .read_exact(&mut buf[fixed_end_offset..end_offset])
            .await
            .map_err(ThriftTcpResponseError::ReadFailed)?;
        if nr + fixed_end_offset != start_offset + 4 + length {
            return Err(ThriftTcpResponseError::NoEnoughDataRead);
        }

        Ok(HeaderTransportResponse {
            seq_id,
            frame_bytes: &buf[fixed_end_offset + var_header_size..],
        })
    }
}
