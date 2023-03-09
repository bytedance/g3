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

mod auth;
pub use auth::{proxy_authenticate_basic, proxy_authorization_basic, www_authenticate_basic};

mod connection;
pub use connection::{connection_as_bytes, connection_with_more_headers};

mod content;
pub use content::{content_length, content_range_overflowed, content_range_sized, content_type};

mod transfer;
pub use transfer::transfer_encoding_chunked;
