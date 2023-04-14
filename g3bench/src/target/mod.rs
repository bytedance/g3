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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use cadence::StatsdClient;
use hdrhistogram::Histogram;
use tokio::signal::unix::SignalKind;
use tokio::sync::{mpsc, Barrier, Semaphore};
use tokio::time::Instant;

use g3_signal::{ActionSignal, SigResult};

use super::ProcArgs;

mod stats;

mod proxy_protocol;
use proxy_protocol::{AppendProxyProtocolArgs, ProxyProtocolArgs};

mod tls;
use tls::{AppendTlsArgs, OpensslTlsClientArgs};

mod http;

pub mod h1;
pub mod h2;
pub mod keyless;
pub mod ssl;

trait BenchHistogram {
    fn refresh(&mut self);
    fn emit(&self, client: &StatsdClient);
    fn summary(&self);

    fn summary_histogram_title(title: &str) {
        println!("{title}");
        println!("                 min      mean[+/-sd]        pct90       max");
    }

    fn summary_newline() {
        println!();
    }

    fn summary_data_line(name: &str, h: &Histogram<u64>) {
        let d_min = h.min();
        let d_mean = h.mean();
        let d_std_dev = h.stdev();
        let d_pct90 = h.value_at_quantile(0.9);
        let d_max = h.max();

        println!(
            "{name:<10} {d_min:>9.3?} {d_mean:>9.3?} {d_std_dev:<9.3?} {d_pct90:>9.3?} {d_max:>9.3?}"
        );
    }

    fn summary_duration_line(name: &str, h: &Histogram<u64>) {
        const NANOS_PER_SEC: f64 = 1_000_000_000.0;

        let t_min = Duration::from_nanos(h.min());
        let t_mean = Duration::from_secs_f64(h.mean() / NANOS_PER_SEC);
        let t_std_dev = Duration::from_secs_f64(h.stdev() / NANOS_PER_SEC);
        let t_pct90 = Duration::from_nanos(h.value_at_quantile(0.9));
        let t_max = Duration::from_nanos(h.max());

        println!(
            "{name:<10} {t_min:>9.3?} {t_mean:>9.3?} {t_std_dev:9.3?} {t_pct90:>9.3?} {t_max:>9.3?}"
        );
    }

    fn summary_total_percentage(h: &Histogram<u64>) {
        macro_rules! print_pct {
            ($pct:literal) => {
                let v = Duration::from_nanos(h.value_at_percentile($pct as f64));
                println!("{:4}% {v:8.3?}", $pct);
            };
        }

        println!("Percentage of the requests served within a certain time");

        print_pct!(50);
        print_pct!(66);
        print_pct!(75);
        print_pct!(80);
        print_pct!(90);
        print_pct!(95);
        print_pct!(98);
        print_pct!(99);
        print_pct!(100);
    }
}

trait BenchRuntimeStats {
    fn emit(&self, client: &StatsdClient);
    fn summary(&self, total_time: Duration);
}

#[async_trait]
trait BenchTaskContext {
    fn mark_task_start(&self);
    fn mark_task_passed(&self);
    fn mark_task_failed(&self);
    async fn run(&mut self, task_id: usize, time_started: Instant) -> anyhow::Result<()>;
}

trait BenchTarget<RS, H, C>
where
    RS: BenchRuntimeStats,
    H: BenchHistogram,
    C: BenchTaskContext,
{
    fn new_context(&self) -> anyhow::Result<C>;
    fn fetch_runtime_stats(&self) -> Arc<RS>;
    fn take_histogram(&mut self) -> Option<H>;

    fn notify_finish(&mut self) {}
}

fn quit_at_sigint(_count: u32) -> SigResult {
    stats::mark_force_quit();
    SigResult::Break
}

async fn run<RS, H, C, T>(mut target: T, proc_args: &ProcArgs) -> anyhow::Result<()>
where
    RS: BenchRuntimeStats + Send + Sync + 'static,
    H: BenchHistogram + Send + 'static,
    C: BenchTaskContext + Send + 'static,
    T: BenchTarget<RS, H, C> + Send + Sync + 'static,
{
    let sync_sem = Arc::new(Semaphore::new(0));
    let sync_barrier = Arc::new(Barrier::new(proc_args.concurrency + 1));
    let (sender, mut receiver) = mpsc::channel::<usize>(proc_args.concurrency);
    let progress = proc_args.new_progress_bar();
    let progress_counter = progress.as_ref().map(|p| p.counter());

    stats::init_global_state(proc_args.requests, proc_args.log_error_count);
    tokio::spawn(
        ActionSignal::new(SignalKind::interrupt(), &quit_at_sigint)
            .map_err(|e| anyhow!("failed to set handler for SIGINT: {e:?}"))?,
    );

    for i in 0..proc_args.concurrency {
        let sem = Arc::clone(&sync_sem);
        let barrier = Arc::clone(&sync_barrier);
        let quit_sender = sender.clone();
        let progress_counter = progress_counter.clone();

        let mut context = target
            .new_context()
            .context(format!("failed to to create context #{i}"))?;

        let task_unconstrained = proc_args.task_unconstrained;
        let rt = super::worker::select_handle(i).unwrap_or_else(tokio::runtime::Handle::current);
        rt.spawn(async move {
            sem.add_permits(1);
            barrier.wait().await;

            let global_state = stats::global_state();
            let mut req_count = 0;
            while let Some(task_id) = global_state.fetch_request() {
                let time_start = Instant::now();
                context.mark_task_start();
                let rt = if task_unconstrained {
                    tokio::task::unconstrained(context.run(task_id, time_start)).await
                } else {
                    context.run(task_id, time_start).await
                };
                match rt {
                    Ok(_) => {
                        context.mark_task_passed();
                        if let Some(c) = progress_counter.as_ref() {
                            c.inc();
                        }
                        global_state.add_passed();
                    }
                    Err(e) => {
                        context.mark_task_failed();
                        if global_state.check_log_error() {
                            eprintln!("! request {task_id} failed: {e:?}\n");
                        }
                        global_state.add_failed();
                    }
                }
                req_count += 1;
            }

            drop(context);
            if let Err(e) = quit_sender.send(req_count).await {
                eprintln!("failed to send quit signal: {e}");
            }
        });
    }
    drop(sender);

    let _run_permit = sync_sem
        .acquire_many(proc_args.concurrency as u32)
        .await
        .context("failed to start all task contexts")?;

    let quit_notifier = Arc::new(AtomicBool::new(false));
    // progress bar
    let progress_bar_handler = if let Some(progress) = progress {
        let handler = progress.spawn(quit_notifier.clone())?;
        Some(handler)
    } else {
        None
    };
    // simple runtime stats
    let runtime_stats_handler =
        if let Some((statsd_client, emit_duration)) = proc_args.new_statsd_client() {
            let runtime_stats = target.fetch_runtime_stats();
            let quit_notifier = quit_notifier.clone();
            let handler = std::thread::Builder::new()
                .name("runtime-stats".to_string())
                .spawn(move || loop {
                    runtime_stats.emit(&statsd_client);
                    statsd_client.flush_sink();

                    if quit_notifier.load(Ordering::Relaxed) {
                        break;
                    }

                    std::thread::sleep(emit_duration);
                })
                .map_err(|e| anyhow!("failed to create runtime stats thread: {e}"))?;
            Some(handler)
        } else {
            None
        };
    // histogram runtime stats
    let histogram_stats_handler = if let Some(mut histogram) = target.take_histogram() {
        let quit_notifier = quit_notifier.clone();
        let thread_builder = std::thread::Builder::new().name("histogram".to_string());
        if let Some((statsd_client, emit_duration)) = proc_args.new_statsd_client() {
            let handler = thread_builder
                .spawn(move || {
                    loop {
                        histogram.refresh();
                        histogram.emit(&statsd_client);

                        if quit_notifier.load(Ordering::Relaxed) {
                            break;
                        }

                        std::thread::sleep(emit_duration);
                    }
                    histogram
                })
                .map_err(|e| anyhow!("failed to create histogram metrics thread: {e}"))?;
            Some(handler)
        } else {
            let handler = thread_builder
                .spawn(move || {
                    loop {
                        histogram.refresh();

                        if quit_notifier.load(Ordering::Relaxed) {
                            break;
                        }

                        std::thread::sleep(Duration::from_millis(100));
                    }
                    histogram
                })
                .map_err(|e| anyhow!("failed to create histogram refresh thread: {e}"))?;
            Some(handler)
        }
    } else {
        None
    };

    let time_start = Instant::now();
    sync_barrier.wait().await;

    if let Some(time_limit) = proc_args.time_limit {
        tokio::spawn(async move {
            tokio::time::sleep(time_limit).await;
            stats::mark_force_quit();
        });
    }

    let mut distribute_histogram = Histogram::<u64>::new(3).unwrap();
    while let Some(req_count) = receiver.recv().await {
        distribute_histogram.record(req_count as u64).unwrap();
    }
    let total_time = time_start.elapsed();

    quit_notifier.store(true, Ordering::Relaxed);

    if let Some(handler) = progress_bar_handler {
        match handler.join() {
            Ok(bar) => bar.finish(),
            Err(e) => eprintln!("error to join progress bar thread: {e:?}"),
        }
    }

    stats::global_state().summary(total_time, &distribute_histogram);
    if let Some(handler) = runtime_stats_handler {
        let _ = handler.join();
    }
    H::summary_newline();
    target.notify_finish();
    target.fetch_runtime_stats().summary(total_time);
    if let Some(handler) = histogram_stats_handler {
        match handler.join() {
            Ok(mut histogram) => {
                histogram.refresh();
                histogram.summary();
            }
            Err(e) => eprintln!("error to join histogram stats thread: {e:?}"),
        }
    }

    Ok(())
}
