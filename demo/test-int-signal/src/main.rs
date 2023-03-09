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

use tokio::signal::unix::SignalKind;

use g3_signal::{ActionSignal, SigResult};

fn do_at_quit(count: u32) -> SigResult {
    match count {
        1 => {
            println!("press 'Ctrl-C' again to quit");
            SigResult::Continue
        }
        _ => {
            println!("quit");
            SigResult::Break
        }
    }
}

#[tokio::main]
async fn main() {
    let sig = ActionSignal::new(SignalKind::interrupt(), &do_at_quit).unwrap();
    println!("SIGINT registered, press 'Ctrl-C' to quit");
    sig.await;
}
