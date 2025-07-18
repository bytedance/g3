/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::Duration;

use g3_io_ext::{LimitedReaderStats, LimitedWriterStats};
use g3_statsd_client::StatsdClient;

use crate::target::BenchRuntimeStats;

#[derive(Default)]
struct TcpIoStats {
    read: AtomicU64,
    write: AtomicU64,
    read_total: AtomicU64,
    write_total: AtomicU64,
}

#[derive(Default)]
pub(crate) struct ThriftRuntimeStats {
    task_total: AtomicU64,
    task_alive: AtomicI64,
    task_passed: AtomicU64,
    task_failed: AtomicU64,
    conn_attempt: AtomicU64,
    conn_attempt_total: AtomicU64,
    conn_success: AtomicU64,
    conn_success_total: AtomicU64,

    io: TcpIoStats,
}

impl ThriftRuntimeStats {
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
}

impl LimitedReaderStats for ThriftRuntimeStats {
    fn add_read_bytes(&self, size: usize) {
        self.io.read.fetch_add(size as u64, Ordering::Relaxed);
    }
}

impl LimitedWriterStats for ThriftRuntimeStats {
    fn add_write_bytes(&self, size: usize) {
        self.io.write.fetch_add(size as u64, Ordering::Relaxed);
    }
}

impl BenchRuntimeStats for ThriftRuntimeStats {
    fn emit(&self, client: &mut StatsdClient) {
        macro_rules! emit_count {
            ($field:ident, $name:literal) => {
                let $field = self.$field.swap(0, Ordering::Relaxed);
                client.count(concat!("thrift.", $name), $field).send();
            };
        }

        let task_alive = self.task_alive.load(Ordering::Relaxed);
        client.gauge("thrift.task.alive", task_alive).send();

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
            ($field:ident, $name:literal) => {
                let $field = self.io.$field.swap(0, Ordering::Relaxed);
                client.count(concat!("thrift.", $name), $field).send();
            };
        }

        emit_io_count!(write, "io.tcp.write");
        self.io.write_total.fetch_add(write, Ordering::Relaxed);
        emit_io_count!(read, "io.tcp.read");
        self.io.read_total.fetch_add(read, Ordering::Relaxed);
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

        println!("# Traffic");
        let total_send =
            self.io.write_total.load(Ordering::Relaxed) + self.io.write.load(Ordering::Relaxed);
        println!("Send bytes:    {total_send}");
        println!("Send rate:     {:.3}B/s", total_send as f64 / total_secs);
        let total_recv =
            self.io.read_total.load(Ordering::Relaxed) + self.io.read.load(Ordering::Relaxed);
        println!("Recv bytes:    {total_recv}");
        println!("Recv rate:     {:.3}B/s", total_recv as f64 / total_secs);
    }
}
