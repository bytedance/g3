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

const MESSAGE_HEADER_LENGTH: usize = 8;
pub(crate) const MESSAGE_PADDED_LENGTH: usize = 1024;
const ITEM_HEADER_LENGTH: usize = 3;

mod request;
pub(crate) use request::KeylessRequest;

mod response;
pub(crate) use response::{
    KeylessDataResponse, KeylessErrorResponse, KeylessPongResponse, KeylessResponse,
};
