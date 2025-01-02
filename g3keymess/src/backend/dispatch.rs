/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::sync::atomic::{AtomicUsize, Ordering};

use openssl::nid::Nid;
use openssl::pkey::Id;
use tokio::sync::mpsc;

use g3_types::sync::GlobalInit;

use super::DispatchedKeylessRequest;

const DEFAULT_COUNTER_SHIFT: u8 = 3;

static DISPATCH_CONTAINER: GlobalInit<DispatcherContainer> = GlobalInit::new(
    DispatcherContainer::with_counter_shift(DEFAULT_COUNTER_SHIFT),
);

struct Dispatcher {
    counter: AtomicUsize,
    counter_shift: u8,
    workers: Vec<mpsc::Sender<DispatchedKeylessRequest>>,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Dispatcher::with_counter_shift(DEFAULT_COUNTER_SHIFT)
    }
}

impl Dispatcher {
    const fn with_counter_shift(shift: u8) -> Self {
        Dispatcher {
            counter: AtomicUsize::new(0),
            counter_shift: shift,
            workers: Vec::new(),
        }
    }

    fn dispatch(&self, req: DispatchedKeylessRequest) -> Result<(), DispatchedKeylessRequest> {
        let cur = self.counter.fetch_add(1, Ordering::Relaxed);
        let id = (cur >> self.counter_shift) % self.workers.len();
        self.workers[id].try_send(req).map_err(|e| e.into_inner())
    }
}

pub struct DispatcherContainer {
    rsa_2048: Dispatcher,
    rsa_3072: Dispatcher,
    rsa_4096: Dispatcher,
    ecdsa_p256: Dispatcher,
    ecdsa_p384: Dispatcher,
    ecdsa_p521: Dispatcher,
}

impl DispatcherContainer {
    const fn with_counter_shift(shift: u8) -> Self {
        DispatcherContainer {
            rsa_2048: Dispatcher::with_counter_shift(shift),
            rsa_3072: Dispatcher::with_counter_shift(shift),
            rsa_4096: Dispatcher::with_counter_shift(shift),
            ecdsa_p256: Dispatcher::with_counter_shift(shift),
            ecdsa_p384: Dispatcher::with_counter_shift(shift),
            ecdsa_p521: Dispatcher::with_counter_shift(shift),
        }
    }

    fn dispatch(&self, req: DispatchedKeylessRequest) -> Result<(), DispatchedKeylessRequest> {
        match req.key.id() {
            Id::RSA => match req.key.size() {
                256 => self.rsa_2048.dispatch(req),
                384 => self.rsa_3072.dispatch(req),
                512 => self.rsa_4096.dispatch(req),
                _ => Err(req),
            },
            Id::EC => {
                let Ok(ec_key) = req.key.ec_key() else {
                    return Err(req);
                };
                match ec_key.group().curve_name() {
                    Some(Nid::X9_62_PRIME256V1) => self.ecdsa_p256.dispatch(req),
                    Some(Nid::SECP384R1) => self.ecdsa_p384.dispatch(req),
                    Some(Nid::SECP521R1) => self.ecdsa_p521.dispatch(req),
                    _ => Err(req),
                }
            }
            _ => Err(req),
        }
    }
}

pub(crate) fn dispatch(req: DispatchedKeylessRequest) -> Result<(), DispatchedKeylessRequest> {
    DISPATCH_CONTAINER.as_ref().dispatch(req)
}

macro_rules! define_register {
    ($method:ident, $field:ident) => {
        pub(super) fn $method(
            rsp_sender: mpsc::Sender<DispatchedKeylessRequest>,
            counter_shift: u8,
        ) {
            DISPATCH_CONTAINER.with_mut(|c| {
                c.$field.counter_shift = counter_shift;
                c.$field.workers.push(rsp_sender);
            });
        }
    };
}

define_register!(register_rsa_2048, rsa_2048);
define_register!(register_rsa_3072, rsa_3072);
define_register!(register_rsa_4096, rsa_4096);
define_register!(register_ecdsa_p256, ecdsa_p256);
define_register!(register_ecdsa_p384, ecdsa_p384);
define_register!(register_ecdsa_p521, ecdsa_p521);
