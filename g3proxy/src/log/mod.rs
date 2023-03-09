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

pub(crate) mod types;

mod shared;

pub(crate) mod audit;
pub(crate) mod escape;
pub(crate) mod inspect;
pub(crate) mod intercept;
pub(crate) mod resolve;
pub(crate) mod task;

const LOG_TYPE_TASK: &str = "Task";
const LOG_TYPE_ESCAPE: &str = "Escape";
const LOG_TYPE_RESOLVE: &str = "Resolve";
const LOG_TYPE_INSPECT: &str = "Inspect";
const LOG_TYPE_INTERCEPT: &str = "Intercept";
