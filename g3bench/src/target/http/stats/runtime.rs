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

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::Duration;

use g3_io_ext::{LimitedReaderStats, LimitedRecvStats, LimitedSendStats, LimitedWriterStats};
use g3_statsd_client::StatsdClient;

use crate::target::BenchRuntimeStats;

#[derive(Default)]
struct HttpUdpIoStats {
    send_bytes: AtomicU64,
    send_packets: AtomicU64,
    send_bytes_total: AtomicU64,
    send_packets_total: AtomicU64,
    recv_bytes: AtomicU64,
    recv_packets: AtomicU64,
    recv_bytes_total: AtomicU64,
    recv_packets_total: AtomicU64,
}

#[derive(Default)]
struct HttpTcpIoStats {
    read: AtomicU64,
    write: AtomicU64,
    read_total: AtomicU64,
    write_total: AtomicU64,
}

enum HttpIoStats {
    Tcp(HttpTcpIoStats),
    #[allow(unused)]
    Udp(HttpUdpIoStats),
}

pub(crate) struct HttpRuntimeStats {
    target: &'static str,
    task_total: AtomicU64,
    task_alive: AtomicI64,
    task_passed: AtomicU64,
    task_failed: AtomicU64,
    conn_attempt: AtomicU64,
    conn_attempt_total: AtomicU64,
    conn_success: AtomicU64,
    conn_success_total: AtomicU64,
    conn_close_error: AtomicU64,
    conn_close_timeout: AtomicU64,

    io: HttpIoStats,
}

impl HttpRuntimeStats {
    pub(crate) fn new_tcp(target: &'static str) -> Self {
        HttpRuntimeStats::with_io(target, HttpIoStats::Tcp(HttpTcpIoStats::default()))
    }

    #[cfg(feature = "quic")]
    pub(crate) fn new_udp(target: &'static str) -> Self {
        HttpRuntimeStats::with_io(target, HttpIoStats::Udp(HttpUdpIoStats::default()))
    }

    fn with_io(target: &'static str, io: HttpIoStats) -> Self {
        HttpRuntimeStats {
            target,
            task_total: AtomicU64::new(0),
            task_alive: AtomicI64::new(0),
            task_passed: AtomicU64::new(0),
            task_failed: AtomicU64::new(0),
            conn_attempt: AtomicU64::new(0),
            conn_attempt_total: AtomicU64::new(0),
            conn_success: AtomicU64::new(0),
            conn_success_total: AtomicU64::new(0),
            conn_close_error: AtomicU64::new(0),
            conn_close_timeout: AtomicU64::new(0),
            io,
        }
    }

    pub(crate) fn add_task_total(&self) {
        self.task_total.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn inc_task_alive(&self) {
        self.task_alive.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn dec_task_alive(&self) {
        self.task_alive.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn add_task_passed(&self) {
        self.task_passed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_task_failed(&self) {
        self.task_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_conn_attempt(&self) {
        self.conn_attempt.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_conn_success(&self) {
        self.conn_success.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_conn_close_fail(&self) {
        self.conn_close_error.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_conn_close_timeout(&self) {
        self.conn_close_timeout.fetch_add(1, Ordering::Relaxed);
    }
}

impl LimitedReaderStats for HttpRuntimeStats {
    fn add_read_bytes(&self, size: usize) {
        if let HttpIoStats::Tcp(tcp) = &self.io {
            tcp.read.fetch_add(size as u64, Ordering::Relaxed);
        }
    }
}

impl LimitedWriterStats for HttpRuntimeStats {
    fn add_write_bytes(&self, size: usize) {
        if let HttpIoStats::Tcp(tcp) = &self.io {
            tcp.write.fetch_add(size as u64, Ordering::Relaxed);
        }
    }
}

impl LimitedSendStats for HttpRuntimeStats {
    fn add_send_bytes(&self, size: usize) {
        if let HttpIoStats::Udp(udp) = &self.io {
            udp.send_bytes.fetch_add(size as u64, Ordering::Relaxed);
        }
    }

    fn add_send_packets(&self, n: usize) {
        if let HttpIoStats::Udp(udp) = &self.io {
            udp.send_packets.fetch_add(n as u64, Ordering::Relaxed);
        }
    }
}

impl LimitedRecvStats for HttpRuntimeStats {
    fn add_recv_bytes(&self, size: usize) {
        if let HttpIoStats::Udp(udp) = &self.io {
            udp.recv_bytes.fetch_add(size as u64, Ordering::Relaxed);
        }
    }

    fn add_recv_packets(&self, n: usize) {
        if let HttpIoStats::Udp(udp) = &self.io {
            udp.recv_packets.fetch_add(n as u64, Ordering::Relaxed);
        }
    }
}

impl BenchRuntimeStats for HttpRuntimeStats {
    fn emit(&self, client: &mut StatsdClient) {
        const TAG_NAME_TARGET: &str = "target";

        macro_rules! emit_count {
            ($field:ident, $name:literal) => {
                let $field = self.$field.swap(0, Ordering::Relaxed);
                client
                    .count(concat!("http.", $name), $field)
                    .with_tag(TAG_NAME_TARGET, self.target)
                    .send();
            };
        }

        let task_alive = self.task_alive.load(Ordering::Relaxed);
        client
            .gauge("http.task.alive", task_alive)
            .with_tag(TAG_NAME_TARGET, self.target)
            .send();

        emit_count!(task_total, "task.total");
        emit_count!(task_passed, "task.passed");
        emit_count!(task_failed, "task.failed");
        emit_count!(conn_attempt, "connection.attempt");
        self.conn_attempt_total
            .fetch_add(conn_attempt, Ordering::Relaxed);
        emit_count!(conn_success, "connection.success");
        self.conn_success_total
            .fetch_add(conn_success, Ordering::Relaxed);

        macro_rules! emit_io_count {
            ($obj:ident, $field:ident, $name:literal) => {
                let $field = $obj.$field.swap(0, Ordering::Relaxed);
                client
                    .count(concat!("http.", $name), $field)
                    .with_tag(TAG_NAME_TARGET, self.target)
                    .send();
            };
        }

        match &self.io {
            HttpIoStats::Tcp(tcp) => {
                emit_io_count!(tcp, write, "io.tcp.write");
                tcp.write_total.fetch_add(write, Ordering::Relaxed);
                emit_io_count!(tcp, read, "io.tcp.read");
                tcp.read_total.fetch_add(read, Ordering::Relaxed);
            }
            HttpIoStats::Udp(udp) => {
                emit_io_count!(udp, send_bytes, "io.udp.send_bytes");
                udp.send_bytes_total
                    .fetch_add(send_bytes, Ordering::Relaxed);
                emit_io_count!(udp, send_packets, "io.udp.send_packets");
                udp.send_packets_total
                    .fetch_add(send_packets, Ordering::Relaxed);
                emit_io_count!(udp, recv_bytes, "io.udp.recv_bytes");
                udp.recv_bytes_total
                    .fetch_add(recv_bytes, Ordering::Relaxed);
                emit_io_count!(udp, recv_packets, "io.udp.recv_packets");
                udp.recv_packets_total
                    .fetch_add(recv_packets, Ordering::Relaxed);
            }
        }
    }

    fn summary(&self, total_time: Duration) {
        let total_secs = total_time.as_secs_f64();

        println!("# Connections");
        let total_attempt = self.conn_attempt_total.load(Ordering::Relaxed)
            + self.conn_attempt.load(Ordering::Relaxed);
        println!("Attempt count: {total_attempt}");
        let total_success = self.conn_success_total.load(Ordering::Relaxed)
            + self.conn_success.load(Ordering::Relaxed);
        println!("Success count: {total_success}");
        println!(
            "Success ratio: {:.2}%",
            (total_success as f64 / total_attempt as f64) * 100.0
        );
        println!("Success rate:  {:.3}/s", total_success as f64 / total_secs);
        let close_error = self.conn_close_error.load(Ordering::Relaxed);
        if close_error > 0 {
            println!("Close error:   {close_error}");
        }
        let close_timeout = self.conn_close_timeout.load(Ordering::Relaxed);
        if close_timeout > 0 {
            println!("Close timeout: {close_timeout}");
        }

        println!("# Traffic");
        match &self.io {
            HttpIoStats::Tcp(tcp) => {
                let total_send =
                    tcp.write_total.load(Ordering::Relaxed) + tcp.write.load(Ordering::Relaxed);
                println!("Send bytes:    {total_send}");
                println!("Send rate:     {:.3}B/s", total_send as f64 / total_secs);
                let total_recv =
                    tcp.read_total.load(Ordering::Relaxed) + tcp.read.load(Ordering::Relaxed);
                println!("Recv bytes:    {total_recv}");
                println!("Recv rate:     {:.3}B/s", total_recv as f64 / total_secs);
            }
            HttpIoStats::Udp(udp) => {
                let total_send_bytes = udp.send_bytes_total.load(Ordering::Relaxed)
                    + udp.send_bytes.load(Ordering::Relaxed);
                println!("Send bytes:    {total_send_bytes}");
                println!(
                    "Send rate:     {:.3}B/s",
                    total_send_bytes as f64 / total_secs
                );
                let total_recv_bytes = udp.recv_bytes_total.load(Ordering::Relaxed)
                    + udp.recv_bytes.load(Ordering::Relaxed);
                println!("Recv bytes:    {total_recv_bytes}");
                println!(
                    "Recv rate:     {:.3}B/s",
                    total_recv_bytes as f64 / total_secs
                );
            }
        }
    }
}
