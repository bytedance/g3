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

use cadence::{Counted, Gauged, StatsdClient};

use g3_io_ext::{LimitedReaderStats, LimitedWriterStats};

use crate::target::BenchRuntimeStats;

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

    tcp_read: AtomicU64,
    tcp_write: AtomicU64,
    tcp_read_total: AtomicU64,
    tcp_write_total: AtomicU64,
}

impl HttpRuntimeStats {
    pub(crate) fn new(target: &'static str) -> Self {
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

            tcp_read: AtomicU64::new(0),
            tcp_write: AtomicU64::new(0),
            tcp_read_total: AtomicU64::new(0),
            tcp_write_total: AtomicU64::new(0),
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
        self.tcp_read.fetch_add(size as u64, Ordering::Relaxed);
    }
}

impl LimitedWriterStats for HttpRuntimeStats {
    fn add_write_bytes(&self, size: usize) {
        self.tcp_write.fetch_add(size as u64, Ordering::Relaxed);
    }
}

impl BenchRuntimeStats for HttpRuntimeStats {
    fn emit(&self, client: &StatsdClient) {
        const TAG_NAME_TARGET: &str = "target";

        macro_rules! emit_count {
            ($field:ident, $name:literal) => {
                let $field = self.$field.swap(0, Ordering::Relaxed);
                let v = i64::try_from($field).unwrap_or(i64::MAX);
                client
                    .count_with_tags(concat!("http.", $name), v)
                    .with_tag(TAG_NAME_TARGET, self.target)
                    .send();
            };
        }

        let task_alive = self.task_alive.load(Ordering::Relaxed);
        client
            .gauge_with_tags("http.task.alive", task_alive as f64)
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
        emit_count!(tcp_write, "io.tcp.write");
        self.tcp_write_total.fetch_add(tcp_write, Ordering::Relaxed);
        emit_count!(tcp_read, "io.tcp.read");
        self.tcp_read_total.fetch_add(tcp_read, Ordering::Relaxed);
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
        let total_send =
            self.tcp_write_total.load(Ordering::Relaxed) + self.tcp_write.load(Ordering::Relaxed);
        println!("Send bytes:    {total_send}");
        println!("Send rate:     {:.3}B/s", total_send as f64 / total_secs);
        let total_recv =
            self.tcp_read_total.load(Ordering::Relaxed) + self.tcp_read.load(Ordering::Relaxed);
        println!("Recv bytes:    {total_recv}");
        println!("Recv rate:     {:.3}B/s", total_recv as f64 / total_secs);
    }
}
