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

pub fn mermaid_graph() -> anyhow::Result<String> {
    let mut content = String::with_capacity(4096);
    let _ = content.write_str("flowchart LR\n%% Paste to https://mermaid.live/ to see the graph\n");

    mermaid_auditor(&mut content);

    mermaid_user_group(&mut content);

    mermaid_resolver(&mut content)?;

    mermaid_escaper(&mut content)?;

    mermaid_server(&mut content)?;

    let _ = content.write_str("Client ===> Server\nEscaper ===> Target\n");

    Ok(content)
}

fn mermaid_auditor(content: &mut String) {
    let _ = content.write_str("  subgraph Auditor\n");
    for config in crate::config::audit::get_all() {
        let name = config.name();
        let _ = writeln!(content, "    auditor_{name}[{name}]");
    }
    let _ = content.write_str("  end\n");
}

fn mermaid_user_group(content: &mut String) {
    let _ = content.write_str("  subgraph UserGroup\n");
    for config in crate::config::auth::get_all() {
        let name = config.name();
        let _ = writeln!(content, "    user_group_{name}[{name}]");
    }
    let _ = content.write_str("  end\n");
}

fn mermaid_resolver(content: &mut String) -> anyhow::Result<()> {
    let all_resolver =
        crate::config::resolver::get_all_sorted().context("failed to get all resolver config")?;

    let _ = content.write_str("  subgraph Resolver\n");
    for c in &all_resolver {
        let name = c.name();
        let _ = writeln!(content, "    resolver_{name}[{name}]");
    }
    for c in &all_resolver {
        if let Some(d) = c.dependent_resolver() {
            let s_name = c.name();
            for v in d {
                let _ = writeln!(content, "    resolver_{s_name} --> resolver_{v}");
            }
        }
    }
    let _ = content.write_str("  end\n");
    Ok(())
}

fn mermaid_escaper(content: &mut String) -> anyhow::Result<()> {
    let all_escaper =
        crate::config::escaper::get_all_sorted().context("failed to get all escaper config")?;

    let _ = content.write_str("  subgraph Escaper\n");
    for c in &all_escaper {
        let name = c.name();
        let _ = writeln!(content, "    escaper_{name}[{name}]");
    }
    for c in &all_escaper {
        if let Some(d) = c.dependent_escaper() {
            let s_name = c.name();
            for v in d {
                let _ = writeln!(content, "    escaper_{s_name} --> escaper_{v}");
            }
        }
    }
    let _ = content.write_str("  end\n");

    for c in &all_escaper {
        let r = c.resolver();
        if !r.is_empty() {
            let _ = writeln!(content, "  escaper_{} -. dns .-> resolver_{r}", c.name());
        }
    }

    Ok(())
}

fn mermaid_server(content: &mut String) -> anyhow::Result<()> {
    let all_server =
        crate::config::server::get_all_sorted().context("failed to get all escaper config")?;

    let _ = content.write_str("  subgraph Server\n");
    for c in &all_server {
        let name = c.name();
        let _ = writeln!(content, "    server_{name}[{name}]");
    }
    for c in &all_server {
        if let Some(d) = c.dependent_server() {
            let s_name = c.name();
            for v in d {
                let _ = writeln!(content, "    server_{s_name} --> server_{v}");
            }
        }
    }
    let _ = content.write_str("  end\n");

    for c in &all_server {
        let s_name = c.name();

        let e = c.escaper();
        if !e.is_empty() {
            let _ = writeln!(content, "  server_{s_name} --> escaper_{e}");
        }

        let u = c.user_group();
        if !u.is_empty() {
            let _ = writeln!(content, "  server_{s_name} o-. auth .-o user_group_{u}");
        }

        let a = c.auditor();
        if !a.is_empty() {
            let _ = writeln!(content, "  server_{s_name} <--> auditor_{a}");
        }
    }

    Ok(())
}
