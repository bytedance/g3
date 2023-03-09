/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::env;
use std::io;
use std::str::FromStr;
use std::sync::Arc;

use futures_util::future::try_join;
use once_cell::sync::Lazy;
use tokio::net::{TcpListener, TcpStream};

use g3_io_ext::{LimitedReader, LimitedWriter};

use test_tcp_relay::stats::{CltStats, TaskStats, UpsStats};

static LISTEN_ADDR: Lazy<String> =
    Lazy::new(|| env::var("TEST_LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:10086".to_string()));
static CONNECT_ADDR: Lazy<String> =
    Lazy::new(|| env::var("TEST_CONNECT_ADDR").unwrap_or_else(|_| "127.0.0.1:5201".to_string()));
static SHIFT_MILLIS_STR: Lazy<String> =
    Lazy::new(|| env::var("TEST_SHIFT_MILLIS").unwrap_or_else(|_| "10".to_string()));
static MAX_BYTES_STR: Lazy<String> =
    Lazy::new(|| env::var("TEST_MAX_BYTES").unwrap_or_else(|_| "1000000".to_string()));
static SHIFT_MILLIS: Lazy<u8> = Lazy::new(|| u8::from_str(SHIFT_MILLIS_STR.as_str()).unwrap_or(10));
static MAX_BYTES: Lazy<usize> =
    Lazy::new(|| usize::from_str(MAX_BYTES_STR.as_str()).unwrap_or(1_000_000));

async fn process_socket(mut clt_stream: TcpStream) -> io::Result<()> {
    let mut ups_stream = TcpStream::connect(CONNECT_ADDR.as_str()).await?;
    println!("new connected task");

    let (clt_r, clt_w) = clt_stream.split();
    let (ups_r, ups_w) = ups_stream.split();

    let task_stats = Arc::new(TaskStats::new());

    let (clt_r_stats, clt_w_stats) = CltStats::new_pair(Arc::clone(&task_stats));
    let mut clt_r = LimitedReader::new(clt_r, *SHIFT_MILLIS, *MAX_BYTES, clt_r_stats);
    let mut clt_w = LimitedWriter::new(clt_w, *SHIFT_MILLIS, *MAX_BYTES, clt_w_stats);

    let (ups_r_stats, ups_w_stats) = UpsStats::new_pair(Arc::clone(&task_stats));
    let mut ups_r = LimitedReader::new(ups_r, *SHIFT_MILLIS, *MAX_BYTES, ups_r_stats);
    let mut ups_w = LimitedWriter::new(ups_w, *SHIFT_MILLIS, *MAX_BYTES, ups_w_stats);

    let clt_to_ups = tokio::io::copy(&mut clt_r, &mut ups_w);
    let ups_to_clt = tokio::io::copy(&mut ups_r, &mut clt_w);

    try_join(clt_to_ups, ups_to_clt).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind(LISTEN_ADDR.as_str()).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = process_socket(stream).await {
                println!("process_socket: {e}");
            }
        });
    }
}
