/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::Context;

use std::fmt::Write;

pub fn graphviz_graph() -> anyhow::Result<String> {
    let mut content = String::with_capacity(4096);
    let _ = content.write_str(
        "strict digraph G {\n  \
            /* append `| dot -Tsvg` to draw a svg file*/\n  \
            concentrate=true;clusterrank=global;ranksep=1\n",
    );

    graphviz_auditor(&mut content);

    graphviz_user_group(&mut content);

    graphviz_resolver(&mut content)?;

    graphviz_escaper(&mut content)?;

    graphviz_server(&mut content)?;

    let _ = content.write_str("}\n");

    Ok(content)
}

fn graphviz_auditor(content: &mut String) {
    let _ = content.write_str("  subgraph cluster_auditor {\n");
    let _ = content.write_str("    graph [style=dashed][label=auditor]\n");
    for config in crate::config::audit::get_all() {
        let name = config.name();
        let _ = writeln!(content, "    auditor_{name} [label={name}]");
    }
    let _ = content.write_str("  };\n");
}

fn graphviz_user_group(content: &mut String) {
    let _ = content.write_str("  subgraph cluster_user_group {\n");
    let _ = content.write_str("    graph [style=dashed][label=user_group]\n");
    for config in crate::config::auth::get_all() {
        let name = config.name();
        let _ = writeln!(content, "    user_group_{name} [label={name};color=purple]");
    }
    let _ = content.write_str("  };\n");
}

fn graphviz_resolver(content: &mut String) -> anyhow::Result<()> {
    let all_resolver =
        crate::config::resolver::get_all_sorted().context("failed to get all resolver config")?;

    let _ = content.write_str("  subgraph cluster_resolver {\n");
    let _ = content.write_str("    graph [style=dashed][label=resolver]\n");
    for c in &all_resolver {
        let name = c.name();
        let _ = writeln!(content, "    resolver_{name} [label={name};color=blue]");
    }
    for c in &all_resolver {
        if let Some(d) = c.dependent_resolver() {
            let s_name = c.name();
            for v in d {
                let _ = writeln!(content, "    resolver_{s_name} -> resolver_{v}");
            }
        }
    }
    let _ = content.write_str("  };\n");
    Ok(())
}

fn graphviz_escaper(content: &mut String) -> anyhow::Result<()> {
    let all_escaper =
        crate::config::escaper::get_all_sorted().context("failed to get all escaper config")?;

    let _ = content.write_str("  subgraph cluster_escaper {\n");
    let _ = content.write_str("    graph [label=escaper]\nnode [group=2]\n");
    for c in &all_escaper {
        let name = c.name();
        let _ = writeln!(content, "    escaper_{name} [label={name};color=red]");
    }
    for c in &all_escaper {
        if let Some(d) = c.dependent_escaper() {
            let s_name = c.name();
            for v in d {
                let _ = writeln!(content, "   escaper_{s_name} -> escaper_{v}");
            }
        }
    }
    let _ = content.write_str("  };\n");

    for c in &all_escaper {
        let r = c.resolver();
        if !r.is_empty() {
            let _ = writeln!(
                content,
                "  escaper_{} -> resolver_{r} [style=dotted]",
                c.name()
            );
        }
    }

    Ok(())
}

fn graphviz_server(content: &mut String) -> anyhow::Result<()> {
    let all_server =
        crate::config::server::get_all_sorted().context("failed to get all escaper config")?;

    let _ = content.write_str("  subgraph cluster_server {\n");
    let _ = content.write_str("    graph [label=server]\nnode [group=1]\n");
    for c in &all_server {
        let name = c.name();
        let _ = writeln!(content, "    server_{name} [label={name};color=green]");
    }
    for c in &all_server {
        if let Some(d) = c.dependent_server() {
            let s_name = c.name();
            for v in d {
                let _ = writeln!(content, "    server_{s_name} -> server_{v}");
            }
        }
    }
    let _ = content.write_str("  };\n");

    for c in &all_server {
        let s_name = c.name();

        let e = c.escaper();
        if !e.is_empty() {
            let _ = writeln!(content, "  server_{s_name} -> escaper_{e}");
        }

        let u = c.user_group();
        if !u.is_empty() {
            let _ = writeln!(
                content,
                "  server_{s_name} -> user_group_{u} [style=dotted]"
            );
        }

        let a = c.auditor();
        if !a.is_empty() {
            let _ = writeln!(content, "  server_{s_name} -> auditor_{a}");
        }
    }

    Ok(())
}
