use super::*;
use std::sync::atomic::Ordering;

#[tokio::test]
#[ignore = "requires cargo setup and codex login; consumes subscription usage"]
async fn real_subscription_codex_cli() {
    let binary = cli::binary().await;
    let gateway = live_gateway::LiveGateway::start().await;
    let report = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("artifacts/e2e/cli/live-results.json");
    std::fs::create_dir_all(report.parent().unwrap()).unwrap();
    let mut results = Vec::new();
    for (model, websocket) in [("gpt-6-sol", false), ("gpt-6-luna", true)] {
        let home = cli::home();
        let work = tempfile::tempdir().unwrap();
        let marker = format!("codex-gateway-{}", uuid::Uuid::new_v4());
        std::fs::write(work.path().join("probe.txt"), &marker).unwrap();
        let config = format!(
            r#"model = {model:?}
model_provider = "gateway"
model_reasoning_effort = "low"
approval_policy = "never"
sandbox_mode = "read-only"
[model_providers.gateway]
name = "OpenAI"
base_url = "{}/backend-api/codex"
wire_api = "responses"
env_key = "CODEX_GATEWAY_API_KEY"
requires_openai_auth = false
supports_websockets = {websocket}
request_max_retries = 0
stream_max_retries = 0
"#,
            gateway.url
        );
        std::fs::write(home.path().join("config.toml"), config).unwrap();
        let before = gateway.transports.as_ref().map(|t| {
            (
                t.http.load(Ordering::Relaxed),
                t.websocket.load(Ordering::Relaxed),
            )
        });
        let mut thread_id = None;
        for resume in [false, true] {
            let mut command = tokio::process::Command::new(&binary);
            command
                .env("CODEX_HOME", home.path())
                .env("CODEX_GATEWAY_API_KEY", &gateway.key)
                .env_remove("OPENAI_API_KEY")
                .env_remove("OPENAI_BASE_URL")
                .env_remove("CODEX_API_KEY")
                .current_dir(work.path())
                .arg("exec")
                .kill_on_drop(true);
            if resume {
                command.arg("resume").arg(thread_id.as_deref().unwrap());
            }
            command.args(["--skip-git-repo-check", "--json"]);
            command.arg(if resume {
                "Reply with the exact content of probe.txt from your previous turn, using remembered context. Do not call tools."
            } else {
                "Use a shell command to read probe.txt in the working directory. Reply only with its exact content. Do not modify files or use the network."
            });
            let output = tokio::time::timeout(Duration::from_secs(180), command.output()).await;
            let mut passed = false;
            let mut tool = false;
            let mut shell_calls = 0;
            let mut turn = false;
            let mut text = false;
            let mut exit_code = None;
            let mut error = None;
            match output {
                Ok(Ok(output)) => {
                    exit_code = output.status.code();
                    let events: Result<Vec<Value>, _> = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .filter(|line| !line.is_empty())
                        .map(serde_json::from_str)
                        .collect();
                    if let Ok(events) = events {
                        for event in &events {
                            match event["type"].as_str() {
                                Some("thread.started") => {
                                    thread_id = event["thread_id"].as_str().map(str::to_owned);
                                }
                                Some("turn.completed") => turn = true,
                                Some("item.completed")
                                    if event["item"]["type"] == "agent_message" =>
                                {
                                    text |= event["item"]["text"]
                                        .as_str()
                                        .is_some_and(|v| v.trim() == marker);
                                }
                                Some("item.completed")
                                    if event["item"]["type"] == "command_execution" =>
                                {
                                    shell_calls += 1;
                                    tool |= event["item"]["exit_code"] == 0
                                        && event["item"]["aggregated_output"]
                                            .as_str()
                                            .is_some_and(|v| v.contains(&marker));
                                }
                                _ => {}
                            }
                        }
                        passed = output.status.success()
                            && turn
                            && text
                            && if resume { shell_calls == 0 } else { tool }
                            && thread_id.is_some();
                    } else {
                        error = Some("invalid CLI JSON event stream");
                    }
                    if !passed && error.is_none() {
                        error = Some("CLI did not complete the expected tool/result/continuation");
                    }
                }
                Ok(Err(_)) => error = Some("could not start CLI"),
                Err(_) => error = Some("CLI exceeded 180 seconds"),
            }
            results.push(json!({"model":model,"websocket_enabled":websocket,"resume":resume,
                "passed":passed,"exit_code":exit_code,"turn_completed":turn,"received_expected_text":text,
                "shell_tool_completed":tool,"shell_calls":shell_calls,"error":error}));
            save(&report, &results, false);
            assert!(
                passed,
                "{model} CLI {} failed; see {} (no private output saved)",
                if resume { "resume" } else { "tool call" },
                report.display()
            );
        }
        if let (Some(t), Some((http_before, ws_before))) = (&gateway.transports, before) {
            let http = t.http.load(Ordering::Relaxed) - http_before;
            let ws = t.websocket.load(Ordering::Relaxed) - ws_before;
            let verified = if websocket {
                ws > 0 && http == 0
            } else {
                http > 0 && ws == 0
            };
            results.push(json!({"model":model,"transport_verified":verified,
                "http_requests":http,"websocket_upgrades":ws,"passed":verified}));
            save(&report, &results, false);
            assert!(verified, "CLI did not exercise the requested transport");
        }
        assert!(
            !home.path().join("auth.json").exists(),
            "downstream CLI must not have subscription credentials"
        );
    }
    save(&report, &results, true);
}

fn save(path: &std::path::Path, results: &[Value], finished: bool) {
    std::fs::write(path, serde_json::to_vec_pretty(&json!({
        "cli_version":codex_api_gateway::CODEX_RELEASE,"codex_revision":codex_api_gateway::CODEX_REV,
        "synthetic":false,"finished":finished,"results":results,
    })).unwrap()).unwrap();
}

#[tokio::test]
#[ignore = "requires cargo setup and codex login; consumes subscription usage for parent and child agents"]
async fn real_subscription_codex_multiagent() {
    use std::collections::HashSet;
    let binary = cli::binary().await;
    let gateway = live_gateway::LiveGateway::start().await;
    let report = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("artifacts/e2e/cli/multiagent-results.json");
    std::fs::create_dir_all(report.parent().unwrap()).unwrap();
    let mut results = Vec::new();
    for (model, websocket) in [("gpt-6-sol", false), ("gpt-6-luna", true)] {
        let home = cli::home();
        let work = tempfile::tempdir().unwrap();
        let markers = [
            format!("alpha-{}", uuid::Uuid::new_v4()),
            format!("beta-{}", uuid::Uuid::new_v4()),
        ];
        for (file, marker) in ["alpha.txt", "beta.txt"].iter().zip(&markers) {
            std::fs::write(work.path().join(file), marker).unwrap();
        }
        let config = format!(
            r#"model = {model:?}
model_provider = "gateway"
model_reasoning_effort = "low"
approval_policy = "never"
sandbox_mode = "read-only"
[features]
multi_agent = true
[agents]
max_threads = 3
max_depth = 1
[model_providers.gateway]
name = "OpenAI"
base_url = "{}/backend-api/codex"
wire_api = "responses"
env_key = "CODEX_GATEWAY_API_KEY"
requires_openai_auth = false
supports_websockets = {websocket}
request_max_retries = 0
stream_max_retries = 0
"#,
            gateway.url
        );
        std::fs::write(home.path().join("config.toml"), config).unwrap();
        let before = gateway.transports.as_ref().map(|t| {
            (
                t.http.load(Ordering::Relaxed),
                t.websocket.load(Ordering::Relaxed),
            )
        });
        let output = tokio::time::timeout(Duration::from_secs(300),
            tokio::process::Command::new(&binary)
                .env("CODEX_HOME", home.path())
                .env("CODEX_GATEWAY_API_KEY", &gateway.key)
                .env_remove("OPENAI_API_KEY").env_remove("OPENAI_BASE_URL").env_remove("CODEX_API_KEY")
                .current_dir(work.path())
                .args(["exec", "--skip-git-repo-check", "--json"])
                .arg("This is a multi-agent integration test. You MUST spawn exactly two independent child agents before waiting for either. Tell the first child to use a shell command to read alpha.txt and return only its exact content. Tell the second child to use a shell command to read beta.txt and return only its exact content. Use the default agent and inherit the current model/provider; do not override models. Children must not spawn further agents. You must NOT read either file yourself or run shell commands yourself. Wait until both children finish, then output the two exact strings they returned, one per line. Do not modify files or use the network.")
                .kill_on_drop(true).output()).await;
        let mut spawned = HashSet::new();
        let mut completed = HashSet::new();
        let mut child_results = [false; 2];
        let mut final_results = false;
        let mut turn_completed = false;
        let mut parent_shell_calls = 0;
        let mut calls = Vec::new();
        let mut exit_ok = false;
        let mut parsed = false;
        let mut root_thread = None;
        if let Ok(Ok(output)) = output {
            exit_ok = output.status.success();
            let events: Result<Vec<Value>, _> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| !line.is_empty())
                .map(serde_json::from_str)
                .collect();
            if let Ok(events) = events {
                parsed = true;
                for event in events {
                    if event["type"] == "thread.started" {
                        root_thread = event["thread_id"].as_str().map(str::to_owned);
                    }
                    if event["type"] == "turn.completed" {
                        turn_completed = true;
                    }
                    if event["type"] != "item.completed" {
                        continue;
                    }
                    let item = &event["item"];
                    match item["type"].as_str() {
                        Some("command_execution") => parent_shell_calls += 1,
                        Some("agent_message") => {
                            if let Some(text) = item["text"].as_str() {
                                final_results |= markers.iter().all(|marker| text.contains(marker));
                            }
                        }
                        Some("collab_tool_call") => {
                            calls.push(json!({"tool":item["tool"],"status":item["status"]}));
                            if item["tool"] == "spawn_agent"
                                && item["status"] == "completed"
                                && let Some(ids) = item["receiver_thread_ids"].as_array()
                            {
                                for id in ids.iter().filter_map(Value::as_str) {
                                    spawned.insert(id.to_owned());
                                }
                            }
                            if let Some(states) = item["agents_states"].as_object() {
                                for (id, state) in states {
                                    if state["status"] == "completed" {
                                        completed.insert(id.clone());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        // V2 child spawns are not consistently projected into exec's JSONL items.
        // Verify the CLI's actual independent child rollouts, including provider,
        // shell execution, completion and returned text, before accepting success.
        let mut child_rollouts_verified = 0;
        let mut child_evidence = Vec::new();
        if let Some(root) = root_thread.as_deref() {
            let mut directories = vec![home.path().join("sessions")];
            while let Some(directory) = directories.pop() {
                let Ok(entries) = std::fs::read_dir(directory) else {
                    continue;
                };
                for entry in entries {
                    let path = entry.unwrap().path();
                    if path.is_dir() {
                        directories.push(path);
                        continue;
                    }
                    if path
                        .extension()
                        .is_none_or(|extension| extension != "jsonl")
                    {
                        continue;
                    }
                    let data = std::fs::read_to_string(path).unwrap();
                    let rows: Vec<Value> = data
                        .lines()
                        .filter(|line| !line.is_empty())
                        .map(|line| serde_json::from_str(line).unwrap())
                        .collect();
                    let Some(meta) = rows
                        .iter()
                        .find(|row| row["type"] == "session_meta")
                        .map(|row| &row["payload"])
                    else {
                        continue;
                    };
                    let parent = meta["parent_thread_id"].as_str().or_else(|| {
                        meta.pointer("/source/subagent/thread_spawn/parent_thread_id")
                            .and_then(Value::as_str)
                    });
                    if parent != Some(root) {
                        continue;
                    }
                    let id = meta["id"].as_str().unwrap();
                    spawned.insert(id.to_owned());
                    let done = rows.iter().any(|row| {
                        row["type"] == "event_msg"
                            && matches!(
                                row["payload"]["type"].as_str(),
                                Some("task_complete" | "turn_complete")
                            )
                    });
                    if done {
                        completed.insert(id.to_owned());
                    }
                    let mut verified = false;
                    for (i, marker) in markers.iter().enumerate() {
                        let direct_shell = rows.iter().any(|row| {
                            row["type"] == "event_msg"
                                && row["payload"]["type"] == "exec_command_end"
                                && row["payload"]["exit_code"] == 0
                                && row["payload"]["aggregated_output"]
                                    .as_str()
                                    .is_some_and(|text| text.contains(marker))
                        });
                        // Code Mode persists the enclosing custom tool call/output,
                        // rather than the nested exec_command_end event.
                        let file = ["alpha.txt", "beta.txt"][i];
                        let code_mode_shell = rows
                            .iter()
                            .filter(|row| {
                                row["type"] == "response_item"
                                    && matches!(
                                        row["payload"]["type"].as_str(),
                                        Some("custom_tool_call" | "function_call")
                                    )
                            })
                            .any(|call| {
                                let payload = &call["payload"];
                                let input = payload["input"]
                                    .as_str()
                                    .or_else(|| payload["arguments"].as_str())
                                    .unwrap_or("");
                                let command = input.contains(file)
                                    && (input.contains("exec_command")
                                        || payload["name"]
                                            .as_str()
                                            .is_some_and(|name| name.ends_with("exec_command")));
                                command
                                    && rows.iter().any(|row| {
                                        row["type"] == "response_item"
                                            && matches!(
                                                row["payload"]["type"].as_str(),
                                                Some(
                                                    "custom_tool_call_output"
                                                        | "function_call_output"
                                                )
                                            )
                                            && row["payload"]["call_id"] == payload["call_id"]
                                            && row["payload"]["output"].to_string().contains(marker)
                                    })
                            });
                        let shell = direct_shell || code_mode_shell;
                        let answer = rows.iter().any(|row| {
                            row["type"] == "response_item"
                                && row["payload"]["type"] == "message"
                                && row["payload"]["role"] == "assistant"
                                && row["payload"]["content"].as_array().is_some_and(|parts| {
                                    parts.iter().any(|part| {
                                        part["text"]
                                            .as_str()
                                            .is_some_and(|text| text.contains(marker))
                                    })
                                })
                        });
                        let matched =
                            done && shell && answer && meta["model_provider"] == "gateway";
                        child_evidence.push(json!({"marker_index":i,"done":done,"shell":shell,"answer":answer,"gateway_provider":meta["model_provider"] == "gateway",
                            "event_types":rows.iter().filter(|row| row["type"] == "event_msg").map(|row|row["payload"]["type"].clone()).collect::<Vec<_>>(),
                            "response_types":rows.iter().filter(|row| row["type"] == "response_item").map(|row|row["payload"]["type"].clone()).collect::<Vec<_>>() }));
                        child_results[i] |= matched;
                        verified |= matched;
                    }
                    if verified {
                        child_rollouts_verified += 1;
                    }
                }
            }
        }
        let transport =
            if let (Some(t), Some((http_before, ws_before))) = (&gateway.transports, before) {
                let http = t.http.load(Ordering::Relaxed) - http_before;
                let ws = t.websocket.load(Ordering::Relaxed) - ws_before;
                Some((
                    http,
                    ws,
                    if websocket {
                        ws >= 3 && http == 0
                    } else {
                        http >= 3 && ws == 0
                    },
                ))
            } else {
                None
            };
        let passed = exit_ok
            && parsed
            && turn_completed
            && spawned.len() == 2
            && child_rollouts_verified == 2
            && spawned.is_subset(&completed)
            && child_results.iter().all(|ok| *ok)
            && final_results
            && parent_shell_calls == 0
            && transport.is_none_or(|(_, _, verified)| verified);
        results.push(json!({"model":model,"websocket":websocket,"passed":passed,
            "exit_success":exit_ok,"events_parsed":parsed,"turn_completed":turn_completed,
            "spawned_children":spawned.len(),"completed_children":spawned.intersection(&completed).count(),
            "child_rollouts_verified":child_rollouts_verified,
            "child_evidence":child_evidence,
            "child_results_verified":child_results,"parent_combined_results":final_results,
            "parent_shell_calls":parent_shell_calls,"transport":transport,"collaboration_calls":calls}));
        save(&report, &results, false);
        assert!(!home.path().join("auth.json").exists());
        assert!(
            passed,
            "{model} multiagent failed; safe report: {}",
            report.display()
        );
    }
    save(&report, &results, true);
}
