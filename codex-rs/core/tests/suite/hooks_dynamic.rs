use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_features::Feature;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use wiremock::MockServer;

const DYNAMIC_TOOL_NAME: &str = "automation_update";
const DYNAMIC_NAMESPACE: &str = "codex_app";
const DYNAMIC_HOOK_NAME: &str = "dynamic__codex_app__automation_update";

fn dynamic_tool(namespace: Option<&str>, name: &str) -> DynamicToolSpec {
    DynamicToolSpec {
        namespace: namespace.map(str::to_string),
        name: name.to_string(),
        description: format!("Dynamic hook test tool for {name}."),
        input_schema: json!({
            "type": "object",
            "properties": {
                "job": { "type": "string" }
            },
            "required": ["job"],
            "additionalProperties": false,
        }),
        defer_loading: false,
    }
}

fn write_pre_tool_use_hook(home: &Path, matcher: &str, reason: &str) -> Result<()> {
    let script_path = home.join("pre_tool_use_hook.py");
    let log_path = home.join("pre_tool_use_hook_log.jsonl");
    let matcher_json = serde_json::to_string(matcher).context("serialize pre matcher")?;
    let reason_json = serde_json::to_string(reason).context("serialize pre reason")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

matcher = {matcher_json}
reason = {reason_json}
payload = json.load(sys.stdin)

with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

print(json.dumps({{
    "hookSpecificOutput": {{
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": reason
    }}
}}))
"#,
        log_path = log_path.display(),
        matcher_json = matcher_json,
        reason_json = reason_json,
    );
    let hooks = json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": matcher,
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running dynamic pre tool use hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write dynamic pre tool use hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_post_tool_use_hook(
    home: &Path,
    matcher: &str,
    additional_context: Option<&str>,
) -> Result<()> {
    let script_path = home.join("post_tool_use_hook.py");
    let log_path = home.join("post_tool_use_hook_log.jsonl");
    let additional_context_json =
        serde_json::to_string(&additional_context).context("serialize post context")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

additional_context = {additional_context_json}
payload = json.load(sys.stdin)

with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

if additional_context is not None:
    print(json.dumps({{
        "hookSpecificOutput": {{
            "hookEventName": "PostToolUse",
            "additionalContext": additional_context
        }}
    }}))
"#,
        log_path = log_path.display(),
        additional_context_json = additional_context_json,
    );
    let hooks = json!({
        "hooks": {
            "PostToolUse": [{
                "matcher": matcher,
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running dynamic post tool use hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write dynamic post tool use hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn write_permission_request_hook(home: &Path, matcher: &str) -> Result<()> {
    let script_path = home.join("permission_request_hook.py");
    let log_path = home.join("permission_request_hook_log.jsonl");
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)

with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

print(json.dumps({{
    "hookSpecificOutput": {{
        "hookEventName": "PermissionRequest",
        "decision": {{
            "behavior": "allow"
        }}
    }}
}}))
"#,
        log_path = log_path.display(),
    );
    let hooks = json!({
        "hooks": {
            "PermissionRequest": [{
                "matcher": matcher,
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running dynamic permission request hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write dynamic permission request hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn read_hook_inputs(home: &Path, log_name: &str) -> Result<Vec<Value>> {
    let log_path = home.join(log_name);
    let contents = match fs::read_to_string(&log_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).with_context(|| format!("read {}", log_path.display())),
    };

    contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse hook log line"))
        .collect()
}

async fn build_dynamic_tool_test<F>(
    server: &MockServer,
    dynamic_tools: Vec<DynamicToolSpec>,
    pre_build_hook: F,
) -> Result<TestCodex>
where
    F: FnOnce(&Path) + Send + 'static,
{
    let base_test = test_codex()
        .with_pre_build_hook(pre_build_hook)
        .with_config(|config| {
            if let Err(err) = config.features.enable(Feature::CodexHooks) {
                panic!("test config should allow enabling codex hooks: {err}");
            }
        })
        .build(server)
        .await?;
    let new_thread = base_test
        .thread_manager
        .start_thread_with_tools(
            base_test.config.clone(),
            dynamic_tools,
            /*persist_extended_history*/ false,
        )
        .await?;
    let mut test = base_test;
    test.codex = new_thread.thread;
    test.session_configured = new_thread.session_configured;
    Ok(test)
}

async fn submit_dynamic_tool_turn(
    test: &TestCodex,
    prompt: &str,
    approval_policy: AskForApproval,
) -> Result<String> {
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
    let session_model = test.session_configured.model.clone();
    test.codex
        .submit(Op::UserTurn {
            environments: None,
            items: vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.config.cwd.to_path_buf(),
            approval_policy,
            approvals_reviewer: None,
            sandbox_policy,
            permission_profile,
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    Ok(wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnStarted(event) => Some(event.turn_id.clone()),
        _ => None,
    })
    .await)
}

async fn wait_for_turn_to_finish_without_dynamic_request(
    test: &TestCodex,
    turn_id: &str,
) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let event = test.codex.next_event().await.context("next event")?;
            match event.msg {
                EventMsg::DynamicToolCallRequest(request) => {
                    anyhow::bail!(
                        "unexpected DynamicToolCallRequest for {} {:?}",
                        request.tool,
                        request.namespace
                    );
                }
                EventMsg::TurnComplete(event) if event.turn_id == turn_id => return Ok(()),
                EventMsg::TurnAborted(event) if event.turn_id.as_deref() == Some(turn_id) => {
                    return Ok(());
                }
                _ => {}
            }
        }
    })
    .await
    .context("timeout waiting for turn to finish")?
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_tool_use_blocks_plain_dynamic_tool_before_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-dynamic-plain";
    let arguments = json!({ "job": "plain" }).to_string();
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, DYNAMIC_TOOL_NAME, &arguments),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "plain dynamic hook blocked it"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let block_reason = "blocked plain dynamic tool";
    let test = build_dynamic_tool_test(
        &server,
        vec![dynamic_tool(/*namespace*/ None, DYNAMIC_TOOL_NAME)],
        move |home| {
            if let Err(err) = write_pre_tool_use_hook(home, DYNAMIC_TOOL_NAME, block_reason) {
                panic!("failed to write plain dynamic pre hook: {err}");
            }
        },
    )
    .await?;

    let turn_id = submit_dynamic_tool_turn(
        &test,
        "call the plain dynamic tool with the pre hook",
        AskForApproval::Never,
    )
    .await?;
    wait_for_turn_to_finish_without_dynamic_request(&test, &turn_id).await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("blocked plain dynamic tool output");
    assert!(
        output.contains(&format!(
            "Tool call blocked by PreToolUse hook: {block_reason}. Tool: {DYNAMIC_TOOL_NAME}"
        )),
        "blocked plain dynamic tool output should mention the reason and tool name",
    );

    let hook_inputs = read_hook_inputs(test.codex_home_path(), "pre_tool_use_hook_log.jsonl")?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        json!({
            "hook_event_name": hook_inputs[0]["hook_event_name"],
            "tool_name": hook_inputs[0]["tool_name"],
            "tool_use_id": hook_inputs[0]["tool_use_id"],
            "tool_input": hook_inputs[0]["tool_input"],
        }),
        json!({
            "hook_event_name": "PreToolUse",
            "tool_name": DYNAMIC_TOOL_NAME,
            "tool_use_id": call_id,
            "tool_input": { "job": "plain" },
        }),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_tool_use_blocks_namespaced_dynamic_tool_with_dynamic_matcher() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "pretooluse-dynamic-namespaced";
    let arguments = json!({ "job": "namespaced" }).to_string();
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    call_id,
                    DYNAMIC_NAMESPACE,
                    DYNAMIC_TOOL_NAME,
                    &arguments,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "namespaced dynamic hook blocked it"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let block_reason = "blocked namespaced dynamic tool";
    let test = build_dynamic_tool_test(
        &server,
        vec![dynamic_tool(Some(DYNAMIC_NAMESPACE), DYNAMIC_TOOL_NAME)],
        move |home| {
            if let Err(err) = write_pre_tool_use_hook(home, DYNAMIC_HOOK_NAME, block_reason) {
                panic!("failed to write namespaced dynamic pre hook: {err}");
            }
        },
    )
    .await?;

    let turn_id = submit_dynamic_tool_turn(
        &test,
        "call the namespaced dynamic tool with the pre hook",
        AskForApproval::Never,
    )
    .await?;
    wait_for_turn_to_finish_without_dynamic_request(&test, &turn_id).await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output_item = requests[1].function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("blocked namespaced dynamic tool output");
    assert!(
        output.contains(&format!(
            "Tool call blocked by PreToolUse hook: {block_reason}. Tool: {DYNAMIC_HOOK_NAME}"
        )),
        "blocked namespaced dynamic tool output should mention the namespaced hook name",
    );

    let hook_inputs = read_hook_inputs(test.codex_home_path(), "pre_tool_use_hook_log.jsonl")?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        json!({
            "hook_event_name": hook_inputs[0]["hook_event_name"],
            "tool_name": hook_inputs[0]["tool_name"],
            "tool_use_id": hook_inputs[0]["tool_use_id"],
            "tool_input": hook_inputs[0]["tool_input"],
        }),
        json!({
            "hook_event_name": "PreToolUse",
            "tool_name": DYNAMIC_HOOK_NAME,
            "tool_use_id": call_id,
            "tool_input": { "job": "namespaced" },
        }),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_tool_use_records_namespaced_dynamic_payload_and_context() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-dynamic-namespaced";
    let arguments = json!({ "job": "post" }).to_string();
    let call_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call_with_namespace(
                call_id,
                DYNAMIC_NAMESPACE,
                DYNAMIC_TOOL_NAME,
                &arguments,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "dynamic post hook context observed"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let post_context = "Remember the dynamic post-tool note.";
    let test = build_dynamic_tool_test(
        &server,
        vec![dynamic_tool(Some(DYNAMIC_NAMESPACE), DYNAMIC_TOOL_NAME)],
        move |home| {
            if let Err(err) = write_post_tool_use_hook(home, DYNAMIC_HOOK_NAME, Some(post_context))
            {
                panic!("failed to write namespaced dynamic post hook: {err}");
            }
        },
    )
    .await?;

    let turn_id = submit_dynamic_tool_turn(
        &test,
        "call the namespaced dynamic tool with the post hook",
        AskForApproval::Never,
    )
    .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::DynamicToolCallRequest(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(request.namespace.as_deref(), Some(DYNAMIC_NAMESPACE));
    assert_eq!(request.tool, DYNAMIC_TOOL_NAME);
    assert_eq!(request.arguments, json!({ "job": "post" }));

    let content_items = vec![
        DynamicToolCallOutputContentItem::InputText {
            text: "done".to_string(),
        },
        DynamicToolCallOutputContentItem::InputImage {
            image_url: "https://example.com/dynamic.png".to_string(),
        },
    ];
    test.codex
        .submit(Op::DynamicToolResponse {
            id: request.call_id.clone(),
            response: DynamicToolResponse {
                content_items: content_items.clone(),
                success: true,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| match event {
        EventMsg::TurnComplete(event) => event.turn_id == turn_id,
        _ => false,
    })
    .await;

    call_mock.single_request();
    let final_request = final_mock.single_request();
    assert!(
        final_request
            .message_input_texts("developer")
            .contains(&post_context.to_string()),
        "follow-up request should include dynamic post tool use additional context",
    );
    assert_eq!(
        final_request.function_call_output(call_id)["output"],
        json!([
            {
                "type": "input_text",
                "text": "done",
            },
            {
                "type": "input_image",
                "image_url": "https://example.com/dynamic.png",
                "detail": "high",
            }
        ]),
    );

    let hook_inputs = read_hook_inputs(test.codex_home_path(), "post_tool_use_hook_log.jsonl")?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        json!({
            "hook_event_name": hook_inputs[0]["hook_event_name"],
            "tool_name": hook_inputs[0]["tool_name"],
            "tool_use_id": hook_inputs[0]["tool_use_id"],
            "tool_input": hook_inputs[0]["tool_input"],
            "tool_response": hook_inputs[0]["tool_response"],
        }),
        json!({
            "hook_event_name": "PostToolUse",
            "tool_name": DYNAMIC_HOOK_NAME,
            "tool_use_id": call_id,
            "tool_input": { "job": "post" },
            "tool_response": [
                {
                    "type": "input_text",
                    "text": "done",
                },
                {
                    "type": "input_image",
                    "image_url": "https://example.com/dynamic.png",
                    "detail": "high",
                }
            ],
        }),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_tool_use_does_not_fire_for_unsuccessful_dynamic_calls() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-dynamic-unsuccessful";
    let arguments = json!({ "job": "fail" }).to_string();
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call_with_namespace(
                call_id,
                DYNAMIC_NAMESPACE,
                DYNAMIC_TOOL_NAME,
                &arguments,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "dynamic failure observed"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let test = build_dynamic_tool_test(
        &server,
        vec![dynamic_tool(Some(DYNAMIC_NAMESPACE), DYNAMIC_TOOL_NAME)],
        move |home| {
            if let Err(err) = write_post_tool_use_hook(
                home,
                DYNAMIC_HOOK_NAME,
                Some("should not reach the model"),
            ) {
                panic!("failed to write unsuccessful dynamic post hook: {err}");
            }
        },
    )
    .await?;

    let turn_id = submit_dynamic_tool_turn(
        &test,
        "call the namespaced dynamic tool and fail it",
        AskForApproval::Never,
    )
    .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::DynamicToolCallRequest(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    test.codex
        .submit(Op::DynamicToolResponse {
            id: request.call_id,
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: "tool failed".to_string(),
                }],
                success: false,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| match event {
        EventMsg::TurnComplete(event) => event.turn_id == turn_id,
        _ => false,
    })
    .await;

    let final_request = final_mock.single_request();
    assert_eq!(
        final_request.function_call_output(call_id)["output"],
        json!("tool failed"),
    );
    assert!(
        !final_request
            .message_input_texts("developer")
            .contains(&"should not reach the model".to_string()),
        "unsuccessful dynamic tools should not inject PostToolUse context",
    );
    assert!(
        read_hook_inputs(test.codex_home_path(), "post_tool_use_hook_log.jsonl")?.is_empty(),
        "unsuccessful dynamic tools should not trigger PostToolUse hooks",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn post_tool_use_does_not_fire_for_canceled_dynamic_calls() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "posttooluse-dynamic-canceled";
    let arguments = json!({ "job": "cancel" }).to_string();
    let call_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call_with_namespace(
                call_id,
                DYNAMIC_NAMESPACE,
                DYNAMIC_TOOL_NAME,
                &arguments,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let test = build_dynamic_tool_test(
        &server,
        vec![dynamic_tool(Some(DYNAMIC_NAMESPACE), DYNAMIC_TOOL_NAME)],
        move |home| {
            if let Err(err) = write_post_tool_use_hook(home, DYNAMIC_HOOK_NAME, Some("ignored")) {
                panic!("failed to write canceled dynamic post hook: {err}");
            }
        },
    )
    .await?;

    let turn_id = submit_dynamic_tool_turn(
        &test,
        "start the namespaced dynamic tool and then interrupt it",
        AskForApproval::Never,
    )
    .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::DynamicToolCallRequest(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(request.call_id, call_id);
    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| match event {
        EventMsg::TurnAborted(event) => event.turn_id.as_deref() == Some(turn_id.as_str()),
        _ => false,
    })
    .await;

    call_mock.single_request();
    assert!(
        read_hook_inputs(test.codex_home_path(), "post_tool_use_hook_log.jsonl")?.is_empty(),
        "canceled dynamic tools should not trigger PostToolUse hooks",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dynamic_tools_do_not_trigger_permission_request_hooks() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "permissionrequest-dynamic";
    let arguments = json!({ "job": "approve-me" }).to_string();
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call_with_namespace(
                call_id,
                DYNAMIC_NAMESPACE,
                DYNAMIC_TOOL_NAME,
                &arguments,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "dynamic tool completed without approval hooks"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let test = build_dynamic_tool_test(
        &server,
        vec![dynamic_tool(Some(DYNAMIC_NAMESPACE), DYNAMIC_TOOL_NAME)],
        move |home| {
            if let Err(err) = write_permission_request_hook(home, DYNAMIC_HOOK_NAME) {
                panic!("failed to write dynamic permission request hook: {err}");
            }
        },
    )
    .await?;

    let turn_id = submit_dynamic_tool_turn(
        &test,
        "run the namespaced dynamic tool under unless-trusted approvals",
        AskForApproval::UnlessTrusted,
    )
    .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::DynamicToolCallRequest(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    test.codex
        .submit(Op::DynamicToolResponse {
            id: request.call_id,
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: "still-ran".to_string(),
                }],
                success: true,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| match event {
        EventMsg::TurnComplete(event) => event.turn_id == turn_id,
        _ => false,
    })
    .await;

    assert_eq!(
        final_mock.single_request().function_call_output(call_id)["output"],
        json!("still-ran"),
    );
    assert!(
        read_hook_inputs(test.codex_home_path(), "permission_request_hook_log.jsonl")?.is_empty(),
        "dynamic tools should not start triggering PermissionRequest hooks in this pass",
    );

    Ok(())
}
