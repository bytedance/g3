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

use ahash::AHashMap;
use smol_str::SmolStr;

use crate::command::Command;
use crate::response::UntaggedResponse;

pub struct CommandPipeline {
    cached_commands: AHashMap<SmolStr, Command>,
    ongoing_command: Option<Command>,
    ongoing_response: Option<UntaggedResponse>,
}

impl Default for CommandPipeline {
    fn default() -> Self {
        CommandPipeline::new()
    }
}

impl CommandPipeline {
    pub fn new() -> Self {
        CommandPipeline::with_capacity(32)
    }

    pub fn with_capacity(cap: usize) -> Self {
        CommandPipeline {
            cached_commands: AHashMap::with_capacity(cap),
            ongoing_command: None,
            ongoing_response: None,
        }
    }

    pub fn insert_completed(&mut self, cmd: Command) -> Option<Command> {
        let tag = cmd.tag.clone();
        self.cached_commands.insert(tag, cmd)
    }

    pub fn remove(&mut self, tag: &SmolStr) -> Option<Command> {
        if let Some(cmd) = self.cached_commands.remove(tag) {
            return Some(cmd);
        };
        if let Some(cmd) = self.ongoing_command.take() {
            if cmd.tag.eq(tag) {
                return Some(cmd);
            } else {
                self.ongoing_command = Some(cmd);
            }
        }
        None
    }

    pub fn set_ongoing_command(&mut self, cmd: Command) {
        self.ongoing_command = Some(cmd);
    }

    pub fn ongoing_command(&mut self) -> Option<&mut Command> {
        self.ongoing_command.as_mut()
    }

    pub fn take_ongoing_command(&mut self) -> Option<Command> {
        self.ongoing_command.take()
    }

    pub fn set_ongoing_response(&mut self, rsp: UntaggedResponse) {
        self.ongoing_response = Some(rsp);
    }

    pub fn ongoing_response(&mut self) -> Option<&mut UntaggedResponse> {
        self.ongoing_response.as_mut()
    }

    pub fn take_ongoing_response(&mut self) -> Option<UntaggedResponse> {
        self.ongoing_response.take()
    }
}
