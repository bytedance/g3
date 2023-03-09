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

#[allow(unused)]
#[derive(Copy, Clone, Debug)]
pub enum Facility {
    // kernel messages (these can't be generated from user processes)
    Kern = 0 << 3,
    // generic user-level messages
    User = 1 << 3,
    // mail subsystem
    Mail = 2 << 3,
    // system daemons without separate facility value
    Daemon = 3 << 3,
    // security/authorization messages
    Auth = 4 << 3,
    // messages generated internally by syslogd(8)
    Syslog = 5 << 3,
    // line printer subsystem
    Lpr = 6 << 3,
    // USENET news subsystem
    News = 7 << 3,
    // UUCP subsystem
    Uucp = 8 << 3,
    // clock daemon (cron and at)
    Cron = 9 << 3,
    // security/authorization messages (private)
    AuthPrivate = 10 << 3,
    // ftp daemon
    Ftp = 11 << 3,
    Local0 = 16 << 3,
    Local1 = 17 << 3,
    Local2 = 18 << 3,
    Local3 = 19 << 3,
    Local4 = 20 << 3,
    Local5 = 21 << 3,
    Local6 = 22 << 3,
    Local7 = 23 << 3,
}

#[allow(unused)]
#[derive(Copy, Clone)]
pub enum Severity {
    // system is unusable
    Emergency,
    // action must be taken immediately
    Alert,
    // critical conditions
    Critical,
    // error conditions
    Error,
    // warning conditions
    Warning,
    // normal, but significant, condition
    Notice,
    // informational message
    Info,
    // debug-level message
    Debug,
}

pub type Priority = u8;
