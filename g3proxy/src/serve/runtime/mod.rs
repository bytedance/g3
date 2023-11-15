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

mod auxiliary;
pub(crate) use auxiliary::AuxiliaryServerConfig;

mod auxiliary_context;
mod auxiliary_quic;
mod auxiliary_tcp;
mod ordinary_tcp;

pub(crate) use auxiliary_context::AuxiliaryRunContext;
pub(crate) use auxiliary_quic::AuxiliaryQuicPortRuntime;
pub(crate) use auxiliary_tcp::AuxiliaryTcpPortRuntime;
pub(crate) use ordinary_tcp::OrdinaryTcpServerRuntime;
