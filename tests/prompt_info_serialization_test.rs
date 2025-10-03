//! Test `PromptInfo` serialization to debug the metadata issue

use pmcp::types::{PromptArgument, PromptInfo};
use pmcp::PromptHandler;

#[test]
fn test_prompt_info_serializes_all_fields() {
    let prompt = PromptInfo {
        name: "test_prompt".to_string(),
        description: Some("A test prompt description".to_string()),
        arguments: Some(vec![PromptArgument {
            name: "arg1".to_string(),
            description: Some("First argument".to_string()),
            required: true,
            completion: None,
        }]),
    };

    let json = serde_json::to_value(&prompt).expect("Should serialize");

    println!(
        "Serialized JSON: {}",
        serde_json::to_string_pretty(&json).unwrap()
    );

    // Verify all fields are present
    assert_eq!(json["name"], "test_prompt");
    assert_eq!(json["description"], "A test prompt description");
    assert!(json["arguments"].is_array(), "arguments should be present");

    let args = json["arguments"].as_array().unwrap();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0]["name"], "arg1");
    assert_eq!(args[0]["description"], "First argument");
    assert_eq!(args[0]["required"], true);
}

#[test]
fn test_prompt_info_with_none_fields() {
    let prompt = PromptInfo {
        name: "minimal_prompt".to_string(),
        description: None,
        arguments: None,
    };

    let json = serde_json::to_value(&prompt).expect("Should serialize");

    println!(
        "Minimal JSON: {}",
        serde_json::to_string_pretty(&json).unwrap()
    );

    // When None, fields should be omitted (skip_serializing_if)
    assert_eq!(json["name"], "minimal_prompt");
    assert!(
        json.get("description").is_none(),
        "description should be omitted when None"
    );
    assert!(
        json.get("arguments").is_none(),
        "arguments should be omitted when None"
    );
}

#[test]
fn test_workflow_prompt_info_round_trip() {
    use pmcp::server::workflow::{SequentialWorkflow, WorkflowPromptHandler};
    use std::collections::HashMap;

    let workflow = SequentialWorkflow::new("add_project_task", "Add a task to a Logseq project")
        .argument("project", "Project name", true)
        .argument("task", "Task description", true);

    let handler = WorkflowPromptHandler::new(workflow, HashMap::new(), HashMap::new());

    let metadata = handler.metadata().expect("Should have metadata");

    println!("\n=== Workflow Metadata ===");
    println!("Name: {}", metadata.name);
    println!("Description: {:?}", metadata.description);
    println!("Arguments: {:?}", metadata.arguments);

    // Serialize to JSON
    let json = serde_json::to_value(&metadata).expect("Should serialize");
    println!(
        "\n=== Serialized JSON ===\n{}",
        serde_json::to_string_pretty(&json).unwrap()
    );

    // Verify
    assert_eq!(json["name"], "add_project_task");
    assert_eq!(
        json["description"], "Add a task to a Logseq project",
        "Description should be serialized!"
    );

    assert!(
        json.get("arguments").is_some(),
        "arguments field should exist"
    );
    assert!(!json["arguments"].is_null(), "arguments should not be null");

    let args = json["arguments"]
        .as_array()
        .expect("arguments should be array");
    assert_eq!(args.len(), 2);
    assert_eq!(args[0]["name"], "project");
    assert_eq!(args[0]["description"], "Project name");
}
