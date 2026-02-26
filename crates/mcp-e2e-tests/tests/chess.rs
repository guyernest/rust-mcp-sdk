//! E2E tests for the Chess MCP App widget.
//!
//! Validates board rendering, piece interaction, move execution,
//! status updates, state persistence, and error handling.

use mcp_e2e_tests::{
    get_tool_call_log, get_widget_state, launch_browser, new_page_with_bridge, start_test_server,
    wait_for_element, wait_for_js_condition,
};
use serde_json::json;

/// Build the initial board state: standard chess starting position.
///
/// Board is an 8x8 array indexed by `[rank][file]` where rank 0 is
/// the white side (bottom). Each cell is either null or `{ type, color }`.
fn initial_board() -> serde_json::Value {
    let mut board = vec![vec![serde_json::Value::Null; 8]; 8];

    // Rank 0: white major pieces
    let white_back = [
        "rook", "knight", "bishop", "queen", "king", "bishop", "knight", "rook",
    ];
    for (file, piece) in white_back.iter().enumerate() {
        board[0][file] = json!({ "type": *piece, "color": "white" });
    }
    // Rank 1: white pawns
    for cell in &mut board[1] {
        *cell = json!({ "type": "pawn", "color": "white" });
    }
    // Ranks 2-5: empty (already null)
    // Rank 6: black pawns
    for cell in &mut board[6] {
        *cell = json!({ "type": "pawn", "color": "black" });
    }
    // Rank 7: black major pieces
    let black_back = [
        "rook", "knight", "bishop", "queen", "king", "bishop", "knight", "rook",
    ];
    for (file, piece) in black_back.iter().enumerate() {
        board[7][file] = json!({ "type": *piece, "color": "black" });
    }

    json!(board)
}

/// Standard mock responses for chess widget tool calls.
fn chess_responses() -> serde_json::Value {
    json!({
        "chess_new_game": {
            "board": initial_board(),
            "turn": "white",
            "history": [],
            "status": "inprogress",
            "castling": { "white": { "kingSide": true, "queenSide": true }, "black": { "kingSide": true, "queenSide": true } }
        },
        "chess_valid_moves": {
            "position": "e2",
            "moves": ["e3", "e4"]
        },
        "chess_move": {
            "success": true,
            "state": {
                "board": initial_board(),
                "turn": "black",
                "history": ["e2e4"],
                "status": "inprogress",
                "castling": { "white": { "kingSide": true, "queenSide": true }, "black": { "kingSide": true, "queenSide": true } }
            }
        }
    })
}

/// Error mock responses for testing graceful error handling.
fn chess_error_responses() -> serde_json::Value {
    json!({
        "chess_new_game": {
            "error": "Server error: failed to initialize game"
        }
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn chess_board_renders_64_squares() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &chess_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/chess/board.html"))
        .await
        .unwrap();

    // Wait for the board to render with squares
    wait_for_js_condition(
        &page,
        "document.querySelectorAll('.square').length === 64",
        10000,
    )
    .await
    .unwrap();

    let count: serde_json::Value = page
        .evaluate("document.querySelectorAll('.square').length")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert_eq!(count.as_i64().unwrap(), 64);
}

#[tokio::test(flavor = "multi_thread")]
async fn chess_initial_status_shows_white_to_move() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &chess_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/chess/board.html"))
        .await
        .unwrap();

    // Wait for status element to update
    wait_for_element(&page, "#status", 10000).await.unwrap();

    // Give the init function time to complete and render
    wait_for_js_condition(
        &page,
        "document.getElementById('status').textContent.includes('White')",
        10000,
    )
    .await
    .unwrap();

    let status: serde_json::Value = page
        .evaluate("document.getElementById('status').textContent")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    let text = status.as_str().unwrap();
    assert!(
        text.contains("White"),
        "Status should contain 'White', got: {text}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn chess_calls_new_game_on_init() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &chess_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/chess/board.html"))
        .await
        .unwrap();

    // Wait for the init tool call to complete
    wait_for_js_condition(&page, "window.mcpBridge.__toolCallLog.length > 0", 10000)
        .await
        .unwrap();

    let log = get_tool_call_log(&page).await.unwrap();
    let has_new_game = log.iter().any(|entry| {
        entry
            .get("name")
            .and_then(|n| n.as_str())
            .is_some_and(|n| n == "chess_new_game")
    });

    assert!(has_new_game, "Expected chess_new_game in tool call log");
}

#[tokio::test(flavor = "multi_thread")]
async fn chess_highlights_selected_piece() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &chess_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/chess/board.html"))
        .await
        .unwrap();

    // Wait for board to be fully rendered
    wait_for_js_condition(
        &page,
        "document.querySelectorAll('.square').length === 64",
        10000,
    )
    .await
    .unwrap();

    // Wait for new game to complete (pieces are rendered)
    wait_for_js_condition(&page, "window.mcpBridge.__toolCallLog.length > 0", 10000)
        .await
        .unwrap();

    // Click on e1 (file=4, rank=0) which is the white king
    // data-file="4" data-rank="0"
    page.evaluate(r#"document.querySelector('.square[data-file="4"][data-rank="0"]').click()"#)
        .await
        .unwrap();

    // Wait for the selected class to appear
    wait_for_js_condition(
        &page,
        r#"document.querySelector('.square[data-file="4"][data-rank="0"]').classList.contains('selected')"#,
        5000,
    )
    .await
    .unwrap();

    let has_selected: serde_json::Value = page
        .evaluate(r#"document.querySelector('.square.selected') !== null"#)
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(has_selected.as_bool().unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn chess_fetches_valid_moves_on_select() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &chess_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/chess/board.html"))
        .await
        .unwrap();

    // Wait for init to complete
    wait_for_js_condition(&page, "window.mcpBridge.__toolCallLog.length > 0", 10000)
        .await
        .unwrap();

    // Click on e2 (file=4, rank=1) which is a white pawn
    page.evaluate(r#"document.querySelector('.square[data-file="4"][data-rank="1"]').click()"#)
        .await
        .unwrap();

    // Wait for chess_valid_moves to be called
    wait_for_js_condition(
        &page,
        r#"window.mcpBridge.__toolCallLog.some(e => e.name === 'chess_valid_moves')"#,
        5000,
    )
    .await
    .unwrap();

    let log = get_tool_call_log(&page).await.unwrap();
    let has_valid_moves = log.iter().any(|entry| {
        entry
            .get("name")
            .and_then(|n| n.as_str())
            .is_some_and(|n| n == "chess_valid_moves")
    });

    assert!(
        has_valid_moves,
        "Expected chess_valid_moves in tool call log"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn chess_makes_move_on_valid_click() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &chess_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/chess/board.html"))
        .await
        .unwrap();

    // Wait for init
    wait_for_js_condition(&page, "window.mcpBridge.__toolCallLog.length > 0", 10000)
        .await
        .unwrap();

    // Click on e2 (pawn) to select it
    page.evaluate(r#"document.querySelector('.square[data-file="4"][data-rank="1"]').click()"#)
        .await
        .unwrap();

    // Wait for valid moves to be fetched
    wait_for_js_condition(
        &page,
        r#"window.mcpBridge.__toolCallLog.some(e => e.name === 'chess_valid_moves')"#,
        5000,
    )
    .await
    .unwrap();

    // Click on e4 (file=4, rank=3) to make the move
    page.evaluate(r#"document.querySelector('.square[data-file="4"][data-rank="3"]').click()"#)
        .await
        .unwrap();

    // Wait for chess_move to be called
    wait_for_js_condition(
        &page,
        r#"window.mcpBridge.__toolCallLog.some(e => e.name === 'chess_move')"#,
        5000,
    )
    .await
    .unwrap();

    let log = get_tool_call_log(&page).await.unwrap();
    let has_move = log.iter().any(|entry| {
        entry
            .get("name")
            .and_then(|n| n.as_str())
            .is_some_and(|n| n == "chess_move")
    });

    assert!(has_move, "Expected chess_move in tool call log");
}

#[tokio::test(flavor = "multi_thread")]
async fn chess_status_updates_after_move() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &chess_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/chess/board.html"))
        .await
        .unwrap();

    // Wait for init
    wait_for_js_condition(&page, "window.mcpBridge.__toolCallLog.length > 0", 10000)
        .await
        .unwrap();

    // Select e2 pawn
    page.evaluate(r#"document.querySelector('.square[data-file="4"][data-rank="1"]').click()"#)
        .await
        .unwrap();

    // Wait for valid moves
    wait_for_js_condition(
        &page,
        r#"window.mcpBridge.__toolCallLog.some(e => e.name === 'chess_valid_moves')"#,
        5000,
    )
    .await
    .unwrap();

    // Click e4 to make move
    page.evaluate(r#"document.querySelector('.square[data-file="4"][data-rank="3"]').click()"#)
        .await
        .unwrap();

    // Wait for status to update to "Black"
    wait_for_js_condition(
        &page,
        "document.getElementById('status').textContent.includes('Black')",
        5000,
    )
    .await
    .unwrap();

    let status: serde_json::Value = page
        .evaluate("document.getElementById('status').textContent")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    let text = status.as_str().unwrap();
    assert!(
        text.contains("Black"),
        "Status should show Black's turn after move, got: {text}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn chess_persists_state_via_bridge() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &chess_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/chess/board.html"))
        .await
        .unwrap();

    // Wait for init and state save
    wait_for_js_condition(&page, "window.mcpBridge.__toolCallLog.length > 0", 10000)
        .await
        .unwrap();

    // The widget calls saveState() after newGame(), which calls setState({ gameState })
    // Give it a moment to propagate
    wait_for_js_condition(
        &page,
        "window.mcpBridge.getState().gameState !== undefined",
        5000,
    )
    .await
    .unwrap();

    let state = get_widget_state(&page).await.unwrap();
    assert!(
        state.get("gameState").is_some(),
        "Widget state should have 'gameState' key"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn chess_new_game_button_resets() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &chess_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/chess/board.html"))
        .await
        .unwrap();

    // Wait for first chess_new_game call (init)
    wait_for_js_condition(&page, "window.mcpBridge.__toolCallLog.length > 0", 10000)
        .await
        .unwrap();

    // Click the New Game button
    page.evaluate("document.getElementById('newGameBtn').click()")
        .await
        .unwrap();

    // Wait for chess_new_game to be called a second time
    wait_for_js_condition(
        &page,
        r#"window.mcpBridge.__toolCallLog.filter(e => e.name === 'chess_new_game').length >= 2"#,
        5000,
    )
    .await
    .unwrap();

    let log = get_tool_call_log(&page).await.unwrap();
    let new_game_count = log
        .iter()
        .filter(|entry| {
            entry
                .get("name")
                .and_then(|n| n.as_str())
                .is_some_and(|n| n == "chess_new_game")
        })
        .count();

    assert!(
        new_game_count >= 2,
        "Expected chess_new_game to be called at least twice, got {new_game_count}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn chess_handles_error_gracefully() {
    let (addr, _handle) = start_test_server().await.unwrap();
    let browser = launch_browser().await.unwrap();
    let page = new_page_with_bridge(&browser, &chess_error_responses())
        .await
        .unwrap();

    page.goto(format!("http://{addr}/chess/board.html"))
        .await
        .unwrap();

    // Wait for the board container to be present (it exists in static HTML)
    wait_for_element(&page, "#board", 10000).await.unwrap();

    // The board should still be visible even if chess_new_game returned an error
    let board_visible: serde_json::Value = page
        .evaluate("document.getElementById('board') !== null")
        .await
        .unwrap()
        .into_value()
        .unwrap();

    assert!(
        board_visible.as_bool().unwrap(),
        "Board should remain visible after error"
    );
}
