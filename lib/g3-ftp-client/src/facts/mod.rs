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

use std::str::FromStr;

use chrono::{DateTime, Utc};
use mime::Mime;

use crate::error::FtpFileFactsParseError;

mod entry_type;
pub(crate) mod time_val;

pub use entry_type::FtpFileEntryType;

pub struct FtpFileFacts {
    entry_path: String,
    entry_type: FtpFileEntryType,
    size: Option<u64>,
    media_type: Option<Mime>,
    modify_time: Option<DateTime<Utc>>,
    create_time: Option<DateTime<Utc>>,
}

impl FtpFileFacts {
    pub(crate) fn new(path: &str) -> Self {
        FtpFileFacts {
            entry_path: path.to_string(),
            entry_type: FtpFileEntryType::Unknown,
            size: None,
            media_type: None,
            modify_time: None,
            create_time: None,
        }
    }

    #[inline]
    pub fn entry_path(&self) -> &str {
        self.entry_path.as_str()
    }

    #[inline]
    pub fn entry_type(&self) -> &FtpFileEntryType {
        &self.entry_type
    }

    #[inline]
    pub fn maybe_file(&self) -> bool {
        self.entry_type.maybe_file()
    }

    #[inline]
    pub fn size(&self) -> Option<u64> {
        self.size
    }

    #[inline]
    pub(crate) fn set_size(&mut self, size: u64) {
        self.size = Some(size);
    }

    #[inline]
    pub fn mtime(&self) -> Option<&DateTime<Utc>> {
        self.modify_time.as_ref()
    }

    #[inline]
    pub(crate) fn set_mtime(&mut self, mtime: DateTime<Utc>) {
        self.modify_time = Some(mtime);
    }

    #[inline]
    pub fn media_type(&self) -> Option<&Mime> {
        self.media_type.as_ref()
    }

    pub(crate) fn parse_line(line: &str) -> Result<Self, FtpFileFactsParseError> {
        if let Some((facts, path)) = line.trim_start().split_once(' ') {
            let mut ff = FtpFileFacts::new(path);

            for fact in facts.split(';') {
                if fact.is_empty() {
                    continue;
                }

                if let Some((key, value)) = fact.split_once('=') {
                    ff.set_fact(key, value)?;
                } else {
                    return Err(FtpFileFactsParseError::NoDelimiterInFact(fact.to_string()));
                }
            }

            Ok(ff)
        } else {
            Err(FtpFileFactsParseError::NoSpaceDelimiter)
        }
    }

    fn set_fact(&mut self, key: &str, value: &str) -> Result<(), FtpFileFactsParseError> {
        match key.to_lowercase().as_str() {
            "type" => self.entry_type = FtpFileEntryType::parse(value),
            "modify" => {
                let dt = time_val::parse_from_str(value)
                    .map_err(FtpFileFactsParseError::InvalidModifyTime)?;
                self.modify_time = Some(dt);
            }
            "create" => {
                let dt = time_val::parse_from_str(value)
                    .map_err(FtpFileFactsParseError::InvalidCreateTime)?;
                self.create_time = Some(dt);
            }
            "size" => {
                let size = u64::from_str(value).map_err(|_| FtpFileFactsParseError::InvalidSize)?;
                self.size = Some(size);
            }
            "media-type" => {
                if let Ok(mime) = value.parse() {
                    self.media_type = Some(mime);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_line() {
        let ff = FtpFileFacts::parse_line("type=pdir;sizd=4096;modify=20210525083610;UNIX.mode=0755;UNIX.uid=0;UNIX.gid=0;unique=804g2; /").unwrap();
        assert_eq!(ff.entry_type, FtpFileEntryType::ParentDir);
        assert!(ff.size.is_none());
    }
}
