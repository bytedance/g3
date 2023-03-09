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

use std::future::poll_fn;
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, ReadBuf};
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::Instant;

static TOTAL_FILLED: AtomicUsize = AtomicUsize::new(0);

struct AlwaysFill {
    c: u8,
}

impl AsyncRead for AlwaysFill {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let remaining = buf.remaining();
        let b = buf.initialize_unfilled_to(remaining);
        b.fill(self.c);
        buf.advance(remaining);
        TOTAL_FILLED.fetch_add(remaining, Ordering::Relaxed);
        Poll::Ready(Ok(()))
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut signal = signal(SignalKind::interrupt()).unwrap();
    tokio::spawn(async move {
        poll_fn(|cx| signal.poll_recv(cx)).await;
        // Crtl-C won't be handled if no yield within copy, use Ctrl-\ to quit
        println!("received interrupt signal");
        std::process::exit(-1);
    });

    tokio::spawn(async {
        let mut reader = AlwaysFill { c: b'A' };
        let mut sink = tokio::io::sink();
        println!("start copy");
        let _ = g3_io_ext::LimitedCopy::new(&mut reader, &mut sink, &Default::default()).await;
        // let _ = tokio::io::copy(&mut reader, &mut sink).await;
    });

    let time_start = Instant::now();
    tokio::time::sleep(Duration::from_secs(4)).await;
    let total_filled = TOTAL_FILLED.load(Ordering::Relaxed);
    println!(
        "exit after {:?}, total filled: {total_filled}",
        time_start.elapsed(),
    );
}
