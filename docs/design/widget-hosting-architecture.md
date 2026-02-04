# Widget Hosting Architecture for Stateless Serverless Backends

**Version:** 1.0
**Date:** 2025-01-11
**Status:** Draft

## Executive Summary

This document explores architectural patterns for building rich, interactive widget UIs (chess boards, maps, forms, dashboards) that work with stateless serverless backends like AWS Lambda. The key insight is that **state management is distributed across three tiers**, and the choice of widget hosting strategy significantly impacts developer experience and user performance.

---

## The Core Challenge

### What We're Building

Interactive ChatGPT Apps widgets that:
- Display complex UIs (React apps, interactive maps, game boards)
- Maintain state across user interactions
- Call backend tools for data and actions
- Work with Lambda's stateless, request-response model

### Why It's Hard

```
Traditional Web App                    ChatGPT App with Lambda
─────────────────────                  ───────────────────────

┌─────────────┐                        ┌─────────────┐
│   Browser   │                        │  ChatGPT    │
│             │                        │  (iframe)   │
│  React App  │◄─── WebSocket ───►     │   Widget    │◄─── No WS ───►  Lambda
│   + State   │                        │   + State?  │                 (stateless)
│             │                        │             │
└─────────────┘                        └─────────────┘

│                                      │
▼                                      ▼
Server keeps                           Each request is
session state                          independent
```

### Key Constraints

1. **Lambda is stateless**: No session, no memory between requests
2. **No WebSocket**: Lambda (via API Gateway HTTP) doesn't maintain connections
3. **Widget in iframe**: Limited communication with host
4. **Bundle size limits**: Can't inline large React apps
5. **Cold starts**: Lambda initialization adds latency

---

## The Three-Tier State Model

### Tier 1: Widget State (ChatGPT-Managed)

```
┌─────────────────────────────────────────────────────────────────┐
│                    ChatGPT Infrastructure                        │
│                                                                  │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │                   Widget State Store                     │   │
│   │                                                          │   │
│   │   conversation_id: "abc123"                              │   │
│   │   widget_state: {                                        │   │
│   │       selectedPiece: "e2",                               │   │
│   │       highlightedMoves: ["e3", "e4"],                    │   │
│   │       boardFlipped: false,                               │   │
│   │       lastMoveAnimating: false                           │   │
│   │   }                                                      │   │
│   │                                                          │   │
│   └─────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              ▼                                   │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │                    Widget (iframe)                       │   │
│   │                                                          │   │
│   │   const state = window.openai.widgetState;              │   │
│   │   window.openai.setWidgetState({ ...state, selected }); │   │
│   │                                                          │   │
│   └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

**Characteristics:**
- Managed entirely by ChatGPT
- Persists across widget re-renders within conversation
- No server involvement
- Perfect for UI state (selections, expanded panels, scroll position)

**Use Cases:**
- Selected items in a list
- Form field values (before submit)
- UI preferences (dark mode toggle)
- Animation states
- Pagination position

### Tier 2: Tool Response Data (Ephemeral)

```
┌─────────────────────────────────────────────────────────────────┐
│                         Tool Call Flow                           │
│                                                                  │
│   Widget                  ChatGPT                  Lambda        │
│     │                        │                        │          │
│     │  callTool("chess_move")│                        │          │
│     │───────────────────────►│   MCP tools/call       │          │
│     │                        │───────────────────────►│          │
│     │                        │                        │          │
│     │                        │                        │ Process  │
│     │                        │                        │ Move     │
│     │                        │                        │          │
│     │                        │   CallToolResult       │          │
│     │                        │◄───────────────────────│          │
│     │                        │   {                    │          │
│     │                        │     structuredContent, │          │
│     │                        │     content,           │          │
│     │                        │     _meta              │          │
│     │                        │   }                    │          │
│     │  toolOutput updated    │                        │          │
│     │◄───────────────────────│                        │          │
│     │                        │                        │          │
│     ▼                        │                        │          │
│   Render                     │                        │          │
│   new state                  │                        │          │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Characteristics:**
- Fresh data on each tool call
- `structuredContent`: Model sees this (keep small)
- `_meta`: Only widget sees this (can be large)
- No automatic persistence

**Use Cases:**
- Current game board state
- Search results
- API responses
- Computed data

### Tier 3: Persistent App State (Server-Side)

```
┌─────────────────────────────────────────────────────────────────┐
│                    Persistent State Architecture                 │
│                                                                  │
│   Lambda                           DynamoDB                      │
│   ┌─────────────────┐             ┌────────────────────────┐    │
│   │                 │             │                        │    │
│   │  async fn       │             │  Table: chess_games    │    │
│   │  chess_move()   │────────────►│                        │    │
│   │  {              │  get_item   │  PK: "game#abc123"     │    │
│   │    let game =   │             │  board: "rnbq..."      │    │
│   │      db.get();  │             │  turn: "white"         │    │
│   │                 │             │  moves: [...]          │    │
│   │    game.move(); │             │  created_at: "..."     │    │
│   │                 │             │  updated_at: "..."     │    │
│   │    db.save();   │────────────►│                        │    │
│   │  }              │  put_item   │                        │    │
│   │                 │             │                        │    │
│   └─────────────────┘             └────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Characteristics:**
- Survives across conversations
- Shared across users (if desired)
- Server-authoritative
- Requires infrastructure

**Use Cases:**
- Game states that persist
- User preferences across sessions
- Document contents
- Audit logs

---

## Widget Hosting Options

### Option 1: Inline HTML (Simple Widgets)

**Architecture:**
```
Lambda returns:
{
    "contents": [{
        "uri": "ui://widget/simple.html",
        "mimeType": "text/html+skybridge",
        "text": "<!DOCTYPE html><html>...full HTML/CSS/JS...</html>"
    }]
}
```

**Pros:**
- Simplest deployment
- No additional infrastructure
- Single artifact

**Cons:**
- Size limited (~100KB practical)
- No code splitting
- Can't use React/Vue easily
- Hard to cache

**Best For:**
- Simple data visualizations
- Basic forms
- Status displays
- Widgets under 50KB

**Example:**
```rust
const SIMPLE_WIDGET: &str = r#"<!DOCTYPE html>
<html>
<head>
    <style>
        .status { padding: 20px; border-radius: 8px; }
        .success { background: #d4edda; color: #155724; }
        .error { background: #f8d7da; color: #721c24; }
    </style>
</head>
<body>
    <div id="status" class="status"></div>
    <script>
        const data = window.openai.toolOutput;
        const el = document.getElementById('status');
        el.className = 'status ' + (data.success ? 'success' : 'error');
        el.textContent = data.message;
    </script>
</body>
</html>"#;
```

### Option 2: CDN-Hosted Bundles (Complex Widgets)

**Architecture:**
```
┌─────────────────────────────────────────────────────────────────┐
│                                                                  │
│   Lambda returns template           CDN hosts bundles            │
│   ┌─────────────────────────┐      ┌─────────────────────────┐  │
│   │ {                       │      │                         │  │
│   │   "text": "             │      │  S3 + CloudFront        │  │
│   │     <html>              │      │                         │  │
│   │       <script           │─────►│  /widgets/              │  │
│   │         src=\"CDN/...\" │      │    chess/               │  │
│   │       />                │      │      1.0.0/             │  │
│   │     </html>"            │      │        app.js           │  │
│   │ }                       │      │        app.css          │  │
│   └─────────────────────────┘      │        chunks/          │  │
│                                    │          vendor.js      │  │
│                                    │          chess-ai.js    │  │
│                                    └─────────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Lambda Template:**
```rust
fn chess_widget_template(version: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="stylesheet" href="https://cdn.example.com/widgets/chess/{version}/app.css">
</head>
<body>
    <div id="root"></div>
    <script type="module" src="https://cdn.example.com/widgets/chess/{version}/app.js"></script>
</body>
</html>"#)
}
```

**Pros:**
- Full React/Vue/Svelte support
- Code splitting and lazy loading
- Aggressive caching
- Version control

**Cons:**
- Separate deployment pipeline
- CSP configuration required
- More infrastructure

**Best For:**
- Complex interactive apps
- Games (chess, puzzles)
- Rich data visualization
- Multi-step workflows

**CDN Setup (S3 + CloudFront):**
```yaml
# CloudFormation
WidgetBucket:
  Type: AWS::S3::Bucket
  Properties:
    BucketName: my-app-widgets
    CorsConfiguration:
      CorsRules:
        - AllowedOrigins: ["https://*.web-sandbox.oaiusercontent.com"]
          AllowedMethods: [GET, HEAD]
          AllowedHeaders: ["*"]

WidgetDistribution:
  Type: AWS::CloudFront::Distribution
  Properties:
    DistributionConfig:
      Origins:
        - DomainName: !GetAtt WidgetBucket.DomainName
          S3OriginConfig:
            OriginAccessIdentity: ...
      DefaultCacheBehavior:
        CachePolicyId: 658327ea-f89d-4fab-a63d-7e88639e58f6  # Managed-CachingOptimized
        ViewerProtocolPolicy: redirect-to-https
```

### Option 3: Amplify Hosting (Managed)

**Architecture:**
```
┌─────────────────────────────────────────────────────────────────┐
│                      AWS Amplify Hosting                         │
│                                                                  │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │                   Amplify Console                        │   │
│   │                                                          │   │
│   │   App: my-chatgpt-widgets                                │   │
│   │   Branch: main → https://main.d123.amplifyapp.com       │   │
│   │   Branch: dev  → https://dev.d123.amplifyapp.com        │   │
│   │                                                          │   │
│   │   ┌─────────────────────────────────────────────────┐   │   │
│   │   │  Build Settings (amplify.yml)                   │   │   │
│   │   │                                                  │   │   │
│   │   │  version: 1                                      │   │   │
│   │   │  frontend:                                       │   │   │
│   │   │    phases:                                       │   │   │
│   │   │      build:                                      │   │   │
│   │   │        commands:                                 │   │   │
│   │   │          - npm ci                                │   │   │
│   │   │          - npm run build                         │   │   │
│   │   │    artifacts:                                    │   │   │
│   │   │      baseDirectory: dist                         │   │   │
│   │   │      files: ['**/*']                             │   │   │
│   │   └─────────────────────────────────────────────────┘   │   │
│   │                                                          │   │
│   └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│   Auto-deploys on git push                                       │
│   Preview URLs for PRs                                           │
│   Automatic HTTPS                                                │
│   Global CDN                                                     │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Pros:**
- Git-based deployment
- Preview environments
- Automatic SSL
- Built-in CI/CD

**Cons:**
- Amplify-specific config
- Less control over caching
- Tied to AWS

**Best For:**
- Teams already using Amplify
- Rapid iteration
- Preview/staging environments

**Integration with Lambda:**
```rust
use pmcp::server::chatgpt::AmplifyWidgetConfig;

let config = AmplifyWidgetConfig {
    app_id: "d123456789".to_string(),
    branch: std::env::var("AMPLIFY_BRANCH").unwrap_or("main".to_string()),
    region: "us-east-1".to_string(),
    widget_path: "/chess".to_string(),
};

// Generate template referencing Amplify-hosted widget
let template = format!(r#"<!DOCTYPE html>
<html>
<head>
    <base href="{}">
</head>
<body>
    <div id="root"></div>
    <script type="module" src="{}/assets/index.js"></script>
</body>
</html>"#, config.cdn_url(), config.cdn_url());
```

### Option 4: Edge Functions (Cloudflare Workers)

**Architecture:**
```
┌─────────────────────────────────────────────────────────────────┐
│                     Cloudflare Platform                          │
│                                                                  │
│   ┌────────────────────────────────────────────────────────┐    │
│   │                   Cloudflare Worker                     │    │
│   │                   (MCP Server)                          │    │
│   │                                                         │    │
│   │   export default {                                      │    │
│   │     async fetch(request, env) {                         │    │
│   │       // Handle MCP requests                            │    │
│   │       // Access KV for state                            │    │
│   │       return new Response(...)                          │    │
│   │     }                                                   │    │
│   │   }                                                     │    │
│   │                                                         │    │
│   └────────────────────────────────────────────────────────┘    │
│                         │               │                        │
│                         ▼               ▼                        │
│   ┌─────────────────────────┐  ┌───────────────────────┐        │
│   │    Workers KV           │  │   Workers Sites       │        │
│   │    (State Store)        │  │   (Widget Hosting)    │        │
│   │                         │  │                       │        │
│   │   game:abc123 → {...}   │  │   /chess/index.html  │        │
│   │   user:xyz789 → {...}   │  │   /chess/app.js      │        │
│   │                         │  │   /map/index.html    │        │
│   └─────────────────────────┘  └───────────────────────┘        │
│                                                                  │
│   All on edge (200+ locations worldwide)                         │
│   Ultra-low latency                                              │
│   No cold starts                                                 │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Pros:**
- No cold starts
- Global edge deployment
- Integrated KV store
- Lower latency than Lambda

**Cons:**
- Different ecosystem
- CPU time limits
- Not AWS-native

**Best For:**
- Latency-sensitive apps
- Global user base
- Teams not tied to AWS

---

## Recommended Architecture: Hybrid Model

For most ChatGPT Apps, we recommend a **hybrid approach**:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Recommended Architecture                          │
│                                                                      │
│                          ChatGPT                                     │
│   ┌───────────────────────────────────────────────────────────┐     │
│   │                    Widget (iframe)                         │     │
│   │                                                            │     │
│   │   ┌────────────────────────────────────────────────────┐  │     │
│   │   │           React App (from CDN)                      │  │     │
│   │   │                                                     │  │     │
│   │   │  ┌─────────────┐  ┌─────────────┐  ┌────────────┐  │  │     │
│   │   │  │ Tier 1:     │  │ Tier 2:     │  │ Actions:   │  │  │     │
│   │   │  │ widgetState │  │ toolOutput  │  │ callTool() │  │  │     │
│   │   │  │ (UI state)  │  │ (data)      │  │            │  │  │     │
│   │   │  └─────────────┘  └─────────────┘  └────────────┘  │  │     │
│   │   │                                          │          │  │     │
│   │   └──────────────────────────────────────────┼──────────┘  │     │
│   │                                              │              │     │
│   └──────────────────────────────────────────────┼──────────────┘     │
│                                                  │                    │
└──────────────────────────────────────────────────┼────────────────────┘
                                                   │
           ┌───────────────────────────────────────┼───────────────────┐
           │                                       │                   │
           ▼                                       ▼                   │
┌──────────────────────┐              ┌──────────────────────┐         │
│   CloudFront CDN     │              │   API Gateway        │         │
│                      │              │                      │         │
│   Widget Bundles     │              │   MCP Endpoint       │         │
│   (versioned)        │              │                      │         │
│                      │              └──────────┬───────────┘         │
│   /v1.2.0/           │                         │                     │
│     chess/           │                         │                     │
│       index.js       │                         ▼                     │
│       index.css      │              ┌──────────────────────┐         │
│     map/             │              │   Lambda (Rust)      │         │
│       index.js       │              │                      │         │
│                      │              │   MCP Server         │         │
└──────────────────────┘              │   Tool Handlers      │         │
           │                          │                      │         │
           │                          └──────────┬───────────┘         │
           │                                     │                     │
           │                                     ▼                     │
           │                          ┌──────────────────────┐         │
           │                          │   DynamoDB           │         │
           │                          │                      │         │
           │                          │   Tier 3: Persistent │         │
           │                          │   App State          │         │
           │                          │                      │         │
           │                          │   - Game states      │         │
           │                          │   - User data        │         │
           │                          │   - Documents        │         │
           │                          └──────────────────────┘         │
           │                                                           │
           └───────────────────── AWS Infrastructure ──────────────────┘
```

### Why This Works

1. **Widget bundles are cacheable**: CDN serves static assets globally with high cache hit rates

2. **Lambda handles business logic**: Each tool call is independent; state comes from DynamoDB

3. **ChatGPT manages UI state**: No server involvement for UI interactions

4. **Clean separation**: Widget development is independent of backend

### Implementation Checklist

- [ ] **CDN Setup**
  - S3 bucket for widget bundles
  - CloudFront distribution with CORS
  - Version-based paths (`/v1.0.0/widget/`)

- [ ] **Lambda MCP Server**
  - Tool handlers
  - DynamoDB integration
  - Template generation

- [ ] **Widget Development**
  - React/Vue app with build pipeline
  - `pmcp-widget-runtime` integration
  - Deploy to S3 on release

- [ ] **CSP Configuration**
  - CDN domain in `resource_domains`
  - API domain in `connect_domains` (if direct calls)

---

## State Synchronization Patterns

### Pattern 1: Optimistic Updates

```javascript
// Widget code
async function makeMove(from, to) {
    // 1. Optimistically update UI
    const optimisticBoard = applyMove(currentBoard, from, to);
    setWidgetState({ ...state, board: optimisticBoard, pending: true });

    try {
        // 2. Call backend
        const result = await window.openai.callTool('chess_move', {
            gameId,
            from,
            to
        });

        // 3. Use authoritative state from server
        // (toolOutput will be updated automatically)
        setWidgetState({ ...state, pending: false });

    } catch (error) {
        // 4. Rollback on failure
        setWidgetState({ ...state, board: currentBoard, pending: false, error });
    }
}
```

### Pattern 2: Server-Authoritative

```javascript
// Widget code
async function makeMove(from, to) {
    // 1. Show loading state
    setWidgetState({ ...state, loading: true });

    // 2. Call backend
    await window.openai.callTool('chess_move', { gameId, from, to });

    // 3. toolOutput now contains new board state
    // Widget re-renders with server state
    setWidgetState({ ...state, loading: false });
}
```

### Pattern 3: Polling for Updates

```javascript
// For multi-player games or collaborative apps
useEffect(() => {
    const interval = setInterval(async () => {
        // Check for updates from other players
        await window.openai.callTool('get_game_state', { gameId });
        // Widget will re-render with new toolOutput
    }, 5000);

    return () => clearInterval(interval);
}, [gameId]);
```

---

## Cost Considerations

| Component | Cost Driver | Optimization |
|-----------|-------------|--------------|
| **Lambda** | Invocations + Duration | Reduce payload size, optimize cold starts |
| **API Gateway** | Requests | N/A (required for Lambda) |
| **DynamoDB** | Read/Write units | Use on-demand, optimize access patterns |
| **CloudFront** | Requests + Data transfer | High cache TTL for widgets |
| **S3** | Storage + Requests | Minimal (small widget bundles) |

**Estimated Monthly Cost (10K users, 100K tool calls):**
- Lambda: ~$5-10
- API Gateway: ~$3-5
- DynamoDB: ~$5-10 (on-demand)
- CloudFront: ~$5-10
- **Total: ~$20-35/month**

---

## Security Considerations

### Widget CSP

```rust
let csp = WidgetCSP::new()
    // Only allow fetching from your API
    .connect("https://api.your-domain.com")
    // Only load assets from your CDN
    .resources("https://cdn.your-domain.com")
    .resources("https://*.oaistatic.com")
    // Don't allow any iframes
    // (omit frame_domains entirely)
```

### Lambda Security

```rust
// Validate all inputs
fn chess_move(args: ChessMoveArgs) -> Result<CallToolResult> {
    // Validate game ID format
    if !is_valid_game_id(&args.game_id) {
        return Err(Error::validation("Invalid game ID"));
    }

    // Validate move format
    if !is_valid_square(&args.from) || !is_valid_square(&args.to) {
        return Err(Error::validation("Invalid move"));
    }

    // Load game and verify ownership/access
    let game = db.get_game(&args.game_id).await?;

    // ... process move
}
```

### Sensitive Data Handling

```rust
// Never put sensitive data in structuredContent (model sees it)
CallToolResult {
    structured_content: Some(json!({
        "gameId": game.id,
        "board": game.public_board_state(),
        "turn": game.current_turn(),
    })),
    // Put detailed data in _meta (widget only)
    _meta: Some(json!({
        "fullAnalysis": game.engine_analysis(),
        "moveTimings": game.move_durations(),
        "chatHistory": game.player_chat(),
    }).as_object().cloned()),
    ..Default::default()
}
```

---

## Next Steps

1. **Choose hosting strategy** based on widget complexity
2. **Set up CDN** for complex widgets
3. **Implement state management** using the three-tier model
4. **Configure CSP** for security
5. **Deploy and test** with ChatGPT Apps

See also:
- `chatgpt-apps-integration.md` - Overall design
- `chatgpt-apps-implementation-plan.md` - Implementation phases
