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

use anyhow::Context;

use std::fmt::Write;

pub fn plantuml_graph() -> anyhow::Result<String> {
    let mut content = String::with_capacity(4096);
    let _ = content.write_str("@startuml\n");

    plantuml_auditor(&mut content);

    plantuml_user_group(&mut content);

    plantuml_resolver(&mut content)?;

    plantuml_escaper(&mut content)?;

    plantuml_server(&mut content)?;

    let _ = content.write_str("hide @unlinked\n@enduml\n");

    Ok(content)
}

fn plantuml_auditor(content: &mut String) {
    let _ = content.write_str("package Auditor {\n");
    for config in crate::config::audit::get_all() {
        let name = config.name();
        let _ = writeln!(content, "  component [{name}] as auditor_{name}",);
    }
    let _ = content.write_str("}\n");
}

fn plantuml_user_group(content: &mut String) {
    let _ = content.write_str("package UserGroup {\n");
    for config in crate::config::auth::get_all() {
        let name = config.name();
        let _ = writeln!(content, "  component [{name}] as user_group_{name}",);
    }
    let _ = content.write_str("}\n");
}

fn plantuml_resolver(content: &mut String) -> anyhow::Result<()> {
    let all_resolver =
        crate::config::resolver::get_all_sorted().context("failed to get all resolver config")?;

    let _ = content.write_str("package Resolver {\n");
    for c in &all_resolver {
        let name = c.name();
        let _ = writeln!(content, "  component [{name}] as resolver_{name}",);
    }
    for c in &all_resolver {
        if let Some(d) = c.dependent_resolver() {
            let s_name = c.name();
            for v in d {
                let _ = writeln!(content, "  resolver_{s_name} --> resolver_{v}",);
            }
        }
    }
    let _ = content.write_str("}\n");
    Ok(())
}

fn plantuml_escaper(content: &mut String) -> anyhow::Result<()> {
    let all_escaper =
        crate::config::escaper::get_all_sorted().context("failed to get all escaper config")?;

    let _ = content.write_str("package Escaper {\n");
    for c in &all_escaper {
        let name = c.name();
        let _ = writeln!(content, "  component [{name}] as escaper_{name}",);
    }
    for c in &all_escaper {
        if let Some(d) = c.dependent_escaper() {
            let s_name = c.name();
            for v in d {
                let _ = writeln!(content, "  escaper_{s_name} --> escaper_{v}",);
            }
        }
    }
    let _ = content.write_str("}\n");

    for c in &all_escaper {
        let r = c.resolver();
        if !r.is_empty() {
            let _ = writeln!(content, "escaper_{} ..> resolver_{r}", c.name());
        }
    }

    Ok(())
}

fn plantuml_server(content: &mut String) -> anyhow::Result<()> {
    let all_server =
        crate::config::server::get_all_sorted().context("failed to get all escaper config")?;

    let _ = content.write_str("package Server {\n");
    for c in &all_server {
        let name = c.name();
        let _ = writeln!(content, "  component [{name}] as server_{name}",);
    }
    for c in &all_server {
        if let Some(d) = c.dependent_server() {
            for v in d {
                let s_name = c.name();
                let _ = writeln!(content, "  server_{s_name} --> server_{v}",);
            }
        }
    }
    let _ = content.write_str("}\n");

    for c in &all_server {
        let s_name = c.name();

        let e = c.escaper();
        if !e.is_empty() {
            let _ = writeln!(content, "server_{s_name} --> escaper_{e}");
        }

        let u = c.user_group();
        if !u.is_empty() {
            let _ = writeln!(content, "server_{s_name} ..> user_group_{u}");
        }

        let a = c.auditor();
        if !a.is_empty() {
            let _ = writeln!(content, "server_{s_name} --> auditor_{a}");
        }
    }

    Ok(())
}
