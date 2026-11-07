//! The server as an MCP client sees it: the built `calccraft-mcp` started as a child
//! process and spoken to over stdio with rmcp's own client, as Claude Code, Claude Desktop
//! or VS Code start a local server.

use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::TokioChildProcess;
use serde_json::{Value, json};

async fn connect() -> RunningService<RoleClient, ()> {
    let command = tokio::process::Command::new(env!("CARGO_BIN_EXE_calccraft-mcp"));
    let transport = TokioChildProcess::new(command).expect("the server starts");
    ().serve(transport).await.expect("the handshake")
}

async fn call(
    client: &RunningService<RoleClient, ()>,
    tool: &str,
    arguments: Value,
) -> CallToolResult {
    let Value::Object(arguments) = arguments else {
        panic!("arguments are an object");
    };
    client
        .call_tool(CallToolRequestParams::new(tool.to_string()).with_arguments(arguments))
        .await
        .expect("a tool result")
}

/// The structured result of a successful call.
fn structured(result: &CallToolResult) -> &Value {
    assert_ne!(result.is_error, Some(true), "{result:?}");
    result
        .structured_content
        .as_ref()
        .expect("structured content")
}

/// The message of a failed call, which the agent reads to correct itself.
fn error_text(result: &CallToolResult) -> String {
    assert_eq!(result.is_error, Some(true), "{result:?}");
    let content = serde_json::to_value(&result.content).unwrap();
    content[0]["text"].as_str().expect("a text").to_string()
}

#[tokio::test]
async fn introduces_itself_and_lists_its_tools() {
    let client = connect().await;
    let info = client.peer_info().expect("server info");
    let info = serde_json::to_value(&*info).unwrap();
    assert_eq!(info["serverInfo"]["name"], "calccraft-mcp");
    assert_eq!(info["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(info["instructions"].as_str().unwrap().contains("calculate"));

    let tools = client.list_all_tools().await.expect("tools/list");
    let tools = serde_json::to_value(&tools).unwrap();
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    for name in [
        "analyze_function",
        "calculate",
        "convert_units",
        "date_add",
        "date_difference",
        "programmer",
    ] {
        assert!(names.contains(&name), "{name} in {names:?}");
    }
    for tool in tools.as_array().unwrap() {
        let name = &tool["name"];
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{name}");
        assert_eq!(tool["annotations"]["destructiveHint"], false, "{name}");
        assert_eq!(tool["annotations"]["idempotentHint"], true, "{name}");
        assert_eq!(tool["annotations"]["openWorldHint"], false, "{name}");
        assert_eq!(tool["inputSchema"]["type"], "object", "{name}");
        assert_eq!(tool["outputSchema"]["type"], "object", "{name}");
        assert!(tool["description"].as_str().unwrap().len() > 80, "{name}");
    }
    let calculate = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "calculate")
        .unwrap();
    assert_eq!(calculate["inputSchema"]["required"], json!(["expression"]));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn every_tool_answers() {
    let client = connect().await;

    let result = call(&client, "calculate", json!({ "expression": "1234.56 / 7" })).await;
    let value = structured(&result);
    assert_eq!(value["result"], "176.36571428571428571428571428571");
    assert_eq!(value["fraction"], "30864/175");
    // The same JSON as text, for clients without structured content.
    let text = serde_json::to_value(&result.content).unwrap();
    assert!(text[0]["text"].as_str().unwrap().contains("30864/175"));

    let result = call(
        &client,
        "calculate",
        json!({ "expression": "sin(pi/6)", "angle": "radians" }),
    )
    .await;
    assert_eq!(structured(&result)["result"], "0.5");

    let result = call(
        &client,
        "calculate",
        json!({ "expression": "2 + 3 * 4", "mode": "standard" }),
    )
    .await;
    assert_eq!(structured(&result)["result"], "20");

    let result = call(
        &client,
        "programmer",
        json!({ "expression": "FF AND 3C", "radix": "hex", "word_size": "byte" }),
    )
    .await;
    let value = structured(&result);
    assert_eq!((&value["hex"], &value["dec"]), (&json!("3C"), &json!("60")));

    let result = call(
        &client,
        "convert_units",
        json!({ "category": "length", "value": "5", "from": "km", "to": "mi" }),
    )
    .await;
    assert_eq!(structured(&result)["value"], "3.106856");

    let result = call(
        &client,
        "date_difference",
        json!({ "from": "2026-01-01", "to": "2026-10-18" }),
    )
    .await;
    let value = structured(&result);
    assert_eq!(value["total_days"], 290);
    assert_eq!(value["description"], "9 months, 2 weeks, 3 days");

    let result = call(
        &client,
        "date_add",
        json!({ "date": "2026-01-31", "months": 1 }),
    )
    .await;
    assert_eq!(structured(&result)["date"], "2026-02-28");

    let result = call(
        &client,
        "analyze_function",
        json!({ "equation": "y = 1/x" }),
    )
    .await;
    let value = structured(&result);
    assert_eq!(value["domain"], "{x ≠ 0}");
    assert_eq!(value["parity"], "The function is odd.");

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn errors_are_tool_results_the_agent_can_read() {
    let client = connect().await;
    let result = call(&client, "calculate", json!({ "expression": "1/0" })).await;
    assert_eq!(error_text(&result), "Cannot divide by zero");
    let result = call(&client, "calculate", json!({ "expression": "sin 30" })).await;
    assert!(error_text(&result).contains("sin(2)"));
    let result = call(
        &client,
        "convert_units",
        json!({ "category": "length", "value": "1", "from": "parsec", "to": "m" }),
    )
    .await;
    assert!(error_text(&result).contains("Meters (m)"));
    // A long calculation still answers; beyond 3249!, Windows says Overflow.
    let result = call(&client, "calculate", json!({ "expression": "3000!" })).await;
    assert!(
        structured(&result)["result"]
            .as_str()
            .unwrap()
            .ends_with("e+9130")
    );
    let result = call(&client, "calculate", json!({ "expression": "3250!" })).await;
    assert_eq!(error_text(&result), "Overflow");

    // The review's B1: 44 nines wrapped in a release build and panicked in a debug one.
    let nines = "9".repeat(44);
    let result = call(&client, "programmer", json!({ "expression": nines })).await;
    assert!(error_text(&result).contains("doesn't fit into a QWORD"));
    let result = call(
        &client,
        "programmer",
        json!({ "expression": "340282366920938463463374607431768211456" }),
    )
    .await;
    assert!(error_text(&result).contains("doesn't fit"));
    // B2: GB is gigabytes.
    let result = call(
        &client,
        "convert_units",
        json!({ "category": "data", "value": "1", "from": "GB", "to": "MiB" }),
    )
    .await;
    let value = structured(&result);
    assert_eq!(
        (&value["from"], &value["value"]),
        (&json!("Gigabytes"), &json!("953.6743"))
    );
    // I1: the review's 28 levels of parentheses.
    let nested = format!("{}1+1)*2{}", "(".repeat(28), "+1)*2".repeat(27));
    let result = call(&client, "programmer", json!({ "expression": nested })).await;
    assert!(error_text(&result).contains("more than 25 levels"));
    client.cancel().await.unwrap();
}

/// The review's I2: inputs that could be slow answer within the budget, and the server goes
/// on answering.
#[tokio::test]
async fn pathological_inputs_answer_quickly() {
    let client = connect().await;
    let long_sum = format!("1{}", "+1".repeat(255));
    for (tool, arguments) in [
        ("calculate", json!({ "expression": "9^9^9" })),
        ("calculate", json!({ "expression": "7^(2^20)" })),
        ("calculate", json!({ "expression": "(2^9999)!" })),
        ("calculate", json!({ "expression": "3248.5!" })),
        (
            "calculate",
            json!({ "expression": vec!["3248.5!"; 60].join("*") }),
        ),
        (
            "calculate",
            json!({ "expression": "(123456789/987654321)^9999" }),
        ),
        (
            "calculate",
            json!({ "expression": vec!["asinh(1e-9999)"; 30].join("+") }),
        ),
        ("calculate", json!({ "expression": long_sum })),
        (
            "calculate",
            json!({ "expression": format!("{long_sum}+1") }),
        ),
        (
            "calculate",
            json!({ "expression": "(".repeat(200) + "1" + &")".repeat(200) }),
        ),
        ("analyze_function", json!({ "equation": "x^x^x^x^x^x^x^x" })),
        (
            "analyze_function",
            json!({ "equation": vec!["sin(1000x)"; 90].join("+") }),
        ),
        ("analyze_function", json!({ "equation": "x".repeat(2049) })),
        ("programmer", json!({ "expression": "1".repeat(512) })),
    ] {
        let start = std::time::Instant::now();
        let _ = call(&client, tool, arguments.clone()).await;
        assert!(
            start.elapsed() < std::time::Duration::from_secs(10),
            "{tool} {arguments}"
        );
    }
    let result = call(&client, "calculate", json!({ "expression": "1+1" })).await;
    assert_eq!(structured(&result)["result"], "2");
    client.cancel().await.unwrap();
}
