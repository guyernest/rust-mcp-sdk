# Stateless-First Architecture: Eliminating Server-Side State

**Version:** 1.0
**Date:** 2025-01-11
**Status:** Draft

## Executive Summary

This document refines the state management architecture by embracing a **stateless-first** approach. The key insight is that **the MCP client (widget) already holds full state**, and can send complete context with each tool call—eliminating the need for server-side storage in most cases.

---

## The Insight

Traditional web apps store session state on the server because:
1. HTTP is stateless
2. Browsers refresh and lose state
3. Multiple clients need shared state

**But ChatGPT Apps are different:**
1. `widgetState` persists across re-renders (ChatGPT manages it)
2. `toolOutput` contains full response data
3. Widget can send *complete context* with each `callTool()`

This means Lambda can be **truly stateless**—no DynamoDB, no session management, no state synchronization.

---

## Revised State Model

### Before (Three Tiers with DynamoDB)

```
Tier 1: Widget State    → UI preferences, selections
Tier 2: Tool Response   → Current data snapshot
Tier 3: DynamoDB        → "Persistent" app state  ← OFTEN UNNECESSARY
```

### After (Stateless-First)

```
┌─────────────────────────────────────────────────────────────────┐
│  Tier 1: Widget State (ChatGPT-managed)                         │
│                                                                  │
│  - UI state: selections, expanded panels, scroll position       │
│  - App state: current game position, form data, document        │
│  - Persists automatically within conversation                   │
│  - Read: window.openai.widgetState                              │
│  - Write: window.openai.setWidgetState()                        │
│                                                                  │
├─────────────────────────────────────────────────────────────────┤
│  Tier 2: Stateless Tool Calls                                   │
│                                                                  │
│  Request:  Widget sends FULL CONTEXT with each call             │
│  Process:  Lambda processes without any stored state            │
│  Response: Lambda returns COMPLETE NEW STATE                    │
│                                                                  │
│  ┌─────────────┐         ┌─────────────┐         ┌───────────┐ │
│  │   Widget    │─────────│   Lambda    │─────────│  Widget   │ │
│  │ (has state) │ context │ (stateless) │ result  │(new state)│ │
│  └─────────────┘         └─────────────┘         └───────────┘ │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  Tier 3: Persistent Storage (OPTIONAL - only when required)     │
│                                                                  │
│  Add ONLY for:                                                   │
│  - Multi-player: Shared state between different users           │
│  - Cross-conversation: Resume game/document tomorrow            │
│  - Audit/compliance: Legal requirement to log actions           │
│  - Anti-cheat: Server must be authoritative (adversarial)       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Chess Example: Stateless Implementation

### Widget Sends Full Game State

```typescript
// Widget state contains everything
interface ChessState {
    // Complete game representation
    position: string;        // FEN: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 1"
    moves: string[];         // ["e4", "e5", "Nf3"]

    // UI state
    selectedSquare: string | null;
    highlightedMoves: string[];
    boardFlipped: boolean;
}

// When user makes a move
async function makeMove(from: string, to: string) {
    const state = window.openai.widgetState as ChessState;

    // Send COMPLETE context - Lambda needs nothing else
    const result = await window.openai.callTool('chess_move', {
        position: state.position,      // Current board (FEN)
        moves: state.moves,            // Move history
        from,
        to,
        promotion: 'q'                 // If pawn promotion
    });

    // Result contains complete new state
    // Widget updates automatically via toolOutput
}
```

### Lambda: Pure Function, No State

```rust
#[derive(Deserialize)]
struct ChessMoveArgs {
    position: String,      // FEN notation (~70 chars)
    moves: Vec<String>,    // Move history
    from: String,
    to: String,
    promotion: Option<char>,
}

async fn chess_move(args: ChessMoveArgs, _extra: RequestHandlerExtra) -> Result<CallToolResult> {
    // Parse position from FEN (no database lookup!)
    let board = Board::from_fen(&args.position)?;

    // Validate move is legal
    let chess_move = board.parse_move(&args.from, &args.to, args.promotion)?;
    if !board.is_legal(&chess_move) {
        return Ok(CallToolResult::error("Illegal move"));
    }

    // Apply move
    let new_board = board.apply_move(&chess_move);
    let new_fen = new_board.to_fen();

    // Compute game status
    let status = new_board.game_status();  // ongoing, checkmate, stalemate, draw

    // Check for AI opponent move (if enabled)
    let ai_move = if args.play_against_ai && status == GameStatus::Ongoing {
        Some(compute_best_move(&new_board, depth: 4))
    } else {
        None
    };

    // Return COMPLETE new state
    Ok(CallToolResult {
        content: vec![Content::Text {
            text: format!("Moved {} to {}. {}", args.from, args.to, status.description())
        }],
        structured_content: Some(json!({
            "position": new_fen,
            "moves": [&args.moves[..], &[format!("{}{}", args.from, args.to)]].concat(),
            "lastMove": { "from": args.from, "to": args.to },
            "status": status,
            "aiMove": ai_move,
        })),
        _meta: Some(json!({
            "legalMoves": new_board.legal_moves(),
            "capturedPieces": new_board.captured(),
            "evaluation": new_board.evaluate(),
        }).as_object().cloned()),
        ..Default::default()
    })
}
```

### Key Insight: FEN is Only ~70 Characters

Chess position in FEN notation:
```
rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e3 0 1
```

This encodes:
- All 64 squares
- Whose turn
- Castling rights
- En passant square
- Half-move clock
- Full move number

**No database needed** - the complete game state fits in a single string!

---

## Map Example: Stateless Location Selection

### Widget State

```typescript
interface MapState {
    // App state
    selectedLocations: Location[];
    routeWaypoints: Coordinate[];
    mapBounds: BoundingBox;

    // UI state
    zoomLevel: number;
    centerPoint: Coordinate;
    activeLayer: 'satellite' | 'street' | 'terrain';
}

async function addWaypoint(coord: Coordinate) {
    const state = window.openai.widgetState as MapState;

    // Send full route context
    const result = await window.openai.callTool('calculate_route', {
        waypoints: [...state.routeWaypoints, coord],
        preferences: {
            avoidTolls: true,
            avoidHighways: false
        }
    });

    // Lambda returns complete new route - no server state needed
}
```

### Lambda: Pure Route Calculation

```rust
async fn calculate_route(args: RouteArgs, _extra: RequestHandlerExtra) -> Result<CallToolResult> {
    // All inputs come from the request
    let waypoints = &args.waypoints;
    let prefs = &args.preferences;

    // Call external routing API (stateless)
    let route = routing_api::calculate(waypoints, prefs).await?;

    // Return complete route
    Ok(CallToolResult {
        structured_content: Some(json!({
            "waypoints": waypoints,
            "route": route.geometry,
            "distance": route.total_distance,
            "duration": route.total_duration,
            "steps": route.turn_by_turn,
        })),
        ..Default::default()
    })
}
```

---

## Form/Wizard Example: Multi-Step Without Server State

### Widget State

```typescript
interface WizardState {
    currentStep: number;
    formData: {
        step1: { name: string; email: string } | null;
        step2: { company: string; role: string } | null;
        step3: { preferences: string[] } | null;
    };
    validationErrors: Record<string, string>;
}

async function submitStep(stepNum: number, data: object) {
    const state = window.openai.widgetState as WizardState;

    // Send ALL form data for validation
    const result = await window.openai.callTool('validate_wizard_step', {
        currentStep: stepNum,
        allData: {
            ...state.formData,
            [`step${stepNum}`]: data
        }
    });

    // Lambda validates everything, returns next step or completion
}
```

### Lambda: Stateless Validation

```rust
async fn validate_wizard_step(args: WizardArgs, _extra: RequestHandlerExtra) -> Result<CallToolResult> {
    // Validate the submitted step
    let errors = validate_step(args.current_step, &args.all_data)?;

    if !errors.is_empty() {
        return Ok(CallToolResult {
            structured_content: Some(json!({
                "valid": false,
                "errors": errors,
                "currentStep": args.current_step
            })),
            ..Default::default()
        });
    }

    // Check if all steps complete
    if args.current_step == 3 && all_steps_complete(&args.all_data) {
        // Process final submission (could write to DB here if needed)
        let confirmation = process_submission(&args.all_data).await?;

        return Ok(CallToolResult {
            structured_content: Some(json!({
                "complete": true,
                "confirmation": confirmation
            })),
            ..Default::default()
        });
    }

    // Return next step
    Ok(CallToolResult {
        structured_content: Some(json!({
            "valid": true,
            "nextStep": args.current_step + 1,
            "allData": args.all_data  // Echo back for widget state
        })),
        ..Default::default()
    })
}
```

---

## When You Actually Need Server State

### 1. Multi-Player Games (Shared State)

```
Player A (Browser 1)              Player B (Browser 2)
        │                                 │
        │ move: e2-e4                     │
        └─────────────┬───────────────────┘
                      │
                      ▼
              ┌───────────────┐
              │   DynamoDB    │  ← Shared state required
              │   game:xyz    │
              └───────────────┘
                      │
        ┌─────────────┴───────────────────┐
        ▼                                 ▼
   Player A sees              Player B notified
   move confirmed             of opponent's move
```

**Solution:** Use DynamoDB only for the shared game state. Each player's widget state is still local.

### 2. Cross-Conversation Resume

```
Conversation 1 (Monday)          Conversation 2 (Tuesday)
        │                                 │
   "Play chess"                    "Continue my game"
        │                                 │
        ▼                                 ▼
  Game state in                    Need to load from
  widgetState                      persistent storage
        │                                 │
        └──────── Game ID ────────────────┘
                     │
                     ▼
              ┌───────────────┐
              │   DynamoDB    │  ← Cross-session persistence
              │   game:abc    │
              └───────────────┘
```

**Solution:** Save game on explicit "save game" action, load by game ID.

### 3. Anti-Cheat (Adversarial Client)

If the client could cheat by sending fake state:
```
Malicious client: "My position is: I have 5 queens and your king is in checkmate"
```

**Solution:** Server stores authoritative state, validates all moves against it.

### 4. Audit/Compliance

If you legally must log all actions:
```
Financial app: Must log all trades for SEC compliance
Healthcare: Must log all patient data access for HIPAA
```

**Solution:** Write-through to DynamoDB for audit trail.

---

## Decision Framework

```
┌─────────────────────────────────────────────────────────────────┐
│                    DO I NEED SERVER STATE?                       │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Is the data needed across different conversations?      │   │
│  │                                                          │   │
│  │  NO  ──────────────────────────►  STATELESS OK           │   │
│  │  YES ─┐                                                  │   │
│  └───────┼──────────────────────────────────────────────────┘   │
│          │                                                       │
│  ┌───────▼──────────────────────────────────────────────────┐   │
│  │  Is the data shared between different users?              │   │
│  │                                                          │   │
│  │  NO  ──────────────────────────►  Save on explicit       │   │
│  │                                   "save" action only      │   │
│  │  YES ─┐                                                  │   │
│  └───────┼──────────────────────────────────────────────────┘   │
│          │                                                       │
│  ┌───────▼──────────────────────────────────────────────────┐   │
│  │  Real-time sync needed?                                   │   │
│  │                                                          │   │
│  │  NO  ──────────────────────────►  Polling or manual      │   │
│  │                                   refresh                 │   │
│  │  YES ─┐                                                  │   │
│  └───────┼──────────────────────────────────────────────────┘   │
│          │                                                       │
│          ▼                                                       │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  Consider: DynamoDB + AppSync for real-time sync         │   │
│  │  Or: Polling with short intervals                        │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Payload Size Considerations

### Compact Representations

| Data Type | Compact Format | Size |
|-----------|----------------|------|
| Chess position | FEN notation | ~70 bytes |
| Chess game | PGN notation | ~500 bytes (40 moves) |
| Form data | JSON | ~1-5 KB |
| Document | Markdown/JSON | Variable |
| Drawing | SVG paths | Variable |

### When Size Becomes a Problem

If state exceeds reasonable limits (~100KB), consider:

1. **Compression**: gzip JSON payloads
2. **Delta updates**: Send only changes, reconstruct on server
3. **Chunked state**: Split into sections, send relevant chunk
4. **Server cache**: Use temporary cache (Redis) with TTL, not persistent storage

### Example: Large Document Editing

```typescript
// Instead of sending entire document
await callTool('apply_edit', {
    documentId: 'temp-session-123',  // Temporary cache key
    delta: {
        type: 'insert',
        position: 1234,
        text: 'Hello world'
    }
});

// Server uses short-lived cache (not DynamoDB)
// Cache expires when conversation ends
```

---

## Architecture Comparison

### Traditional (Over-Engineered)

```
Widget ──► API Gateway ──► Lambda ──► DynamoDB
                              │
                              ├── Read state
                              ├── Process
                              ├── Write state
                              └── Return

Cost: Lambda + API Gateway + DynamoDB
Complexity: State management, consistency, cold starts with DB
```

### Stateless-First (Recommended)

```
Widget ──► API Gateway ──► Lambda ──► Return
              │               │
              │               └── Pure function:
              │                   Input → Process → Output
              │
              └── Full context in request

Cost: Lambda + API Gateway only
Complexity: Minimal
```

### Hybrid (When Needed)

```
Widget ──► API Gateway ──► Lambda ──┬──► Return (most calls)
                              │     │
                              │     └──► DynamoDB (only when required)
                              │             - Multi-player sync
                              │             - Explicit save
                              │             - Audit log
```

---

## Implementation Guidelines

### Widget: Always Send Full Context

```typescript
// GOOD: Self-contained request
await callTool('process', {
    currentState: window.openai.widgetState,
    action: { type: 'move', from: 'e2', to: 'e4' }
});

// BAD: Assumes server has state
await callTool('process', {
    gameId: 'abc123',  // Server must look this up
    action: { type: 'move', from: 'e2', to: 'e4' }
});
```

### Lambda: Pure Functions

```rust
// GOOD: Pure function
async fn process(args: Args) -> Result<CallToolResult> {
    let state = parse_state(&args.current_state)?;
    let new_state = apply_action(&state, &args.action)?;
    Ok(CallToolResult::with_structured_content(new_state))
}

// BAD: Side effects, external state
async fn process(args: Args) -> Result<CallToolResult> {
    let state = db.get(&args.game_id).await?;  // External state!
    let new_state = apply_action(&state, &args.action)?;
    db.put(&args.game_id, &new_state).await?;  // Side effect!
    Ok(CallToolResult::with_structured_content(new_state))
}
```

### Response: Return Complete State

```rust
// GOOD: Complete state in response
Ok(CallToolResult {
    structured_content: Some(json!({
        "board": new_board.to_fen(),
        "moves": all_moves,
        "status": game_status,
        "legalMoves": legal_moves,
    })),
    ..Default::default()
})

// BAD: Partial state (requires client to merge)
Ok(CallToolResult {
    structured_content: Some(json!({
        "lastMove": "e2-e4",
        "status": "ongoing"
    })),
    ..Default::default()
})
```

---

## Summary

### Default: Stateless

- Widget holds all state in `widgetState`
- Tool calls send complete context
- Lambda is a pure function
- No database needed

### Exception: Add State When Required

- Multi-player shared state
- Cross-conversation persistence
- Audit/compliance requirements
- Anti-cheat (adversarial client)

### Benefits

| Aspect | Stateless | Stateful |
|--------|-----------|----------|
| Cost | Lower (no DynamoDB) | Higher |
| Latency | Lower (no DB round-trip) | Higher |
| Complexity | Minimal | State management |
| Scaling | Infinite (pure functions) | DB bottleneck |
| Testing | Easy (pure functions) | Harder (mocks) |
| Cold starts | Fast | Slower (DB connection) |

---

## Related Documents

- [unified-ui-architecture.md](./unified-ui-architecture.md) - Multi-platform design
- [widget-hosting-architecture.md](./widget-hosting-architecture.md) - Updated with stateless patterns
- [chatgpt-apps-integration.md](./chatgpt-apps-integration.md) - ChatGPT specifics
