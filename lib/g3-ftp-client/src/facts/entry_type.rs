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

use std::fmt;

#[derive(Debug, Eq, PartialEq)]
pub enum FtpFileEntryType {
    Unknown,
    File,
    Directory,
    CurrentDir,
    ParentDir,
    OsType(String),
}

impl fmt::Display for FtpFileEntryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FtpFileEntryType {
    pub(super) fn parse(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "file" => FtpFileEntryType::File,
            "dir" => FtpFileEntryType::Directory,
            "cdir" => FtpFileEntryType::CurrentDir,
            "pdir" => FtpFileEntryType::ParentDir,
            _ => FtpFileEntryType::OsType(value.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            FtpFileEntryType::Unknown => "unknown",
            FtpFileEntryType::File => "file",
            FtpFileEntryType::Directory => "dir",
            FtpFileEntryType::CurrentDir => "cdir",
            FtpFileEntryType::ParentDir => "pdir",
            FtpFileEntryType::OsType(s) => s,
        }
    }

    pub fn is_dir(&self) -> bool {
        matches!(
            self,
            FtpFileEntryType::Directory
                | FtpFileEntryType::CurrentDir
                | FtpFileEntryType::ParentDir
        )
    }

    pub fn maybe_file(&self) -> bool {
        match self {
            FtpFileEntryType::Unknown => true,
            FtpFileEntryType::File => true,
            FtpFileEntryType::Directory => false,
            FtpFileEntryType::CurrentDir => false,
            FtpFileEntryType::ParentDir => false,
            FtpFileEntryType::OsType(_) => true,
        }
    }
}
