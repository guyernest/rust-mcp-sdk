//! Progress Notifications Example
//!
//! This example demonstrates how progress notifications work in MCP.
//! It shows the structure of progress notifications and tokens.

use pmcp::types::{ProgressNotification, ProgressToken};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== MCP Progress Notifications Example ===");

    // Create a progress tracker
    let progress_tracker = Arc::new(Mutex::new(HashMap::<ProgressToken, f64>::new()));

    // Simulate some progress notifications
    let notifications = vec![
        ProgressNotification::new(ProgressToken::String("task-1".to_string()), 10.0, Some("Starting task...".to_string())),
        ProgressNotification::new(ProgressToken::String("task-1".to_string()), 25.0, Some("Processing data...".to_string())),
        ProgressNotification::new(ProgressToken::String("task-1".to_string()), 50.0, Some("Halfway there...".to_string())),
        ProgressNotification::new(ProgressToken::String("task-1".to_string()), 100.0, Some("Task completed!".to_string())),
    ];

    println!("Simulating progress notifications:");

    for notification in notifications {
        // Create a progress bar visualization (assuming progress is 0-100)
        let percentage = notification.progress;
        let progress_bar = "█".repeat((percentage / 5.0) as usize);
        let empty_bar = "░".repeat(20 - (percentage / 5.0) as usize);

        println!(
            "📊 Progress [{}{}] {:.1}% - {} (Token: {:?})",
            progress_bar,
            empty_bar,
            percentage,
            notification.message.as_deref().unwrap_or("Working..."),
            notification.progress_token
        );

        // Track progress
        progress_tracker
            .lock()
            .unwrap()
            .insert(notification.progress_token.clone(), notification.progress);

        // Simulate some delay
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    println!("✅ Progress tracking completed!");

    // Show final state
    let tracker = progress_tracker.lock().unwrap();
    println!("Final progress state: {:?}", *tracker);

    Ok(())
}
