# Course Server Minimal

A minimal MCP server that serves course content, demonstrating content-serving patterns for educational applications.

## Overview

This example shows how to build an MCP server that:
- Loads content from a filesystem (mdBook-compatible structure)
- Exposes chapters as MCP resources with stable URIs
- Provides navigation tools (list_chapters, get_lesson)
- Includes quiz retrieval (answers excluded for security)
- Offers learning prompts (start_learning, review_chapter)

This is a **simplified teaching example**. For the full production implementation with OAuth, progress tracking, and quiz validation, see the [pmcp.run course server](https://github.com/your-org/pmcp-run/tree/main/built-in/mdbook-course).

## Architecture

```
┌─────────────────────────────────────────────┐
│            course-server-minimal             │
│  ┌────────────┐  ┌────────────┐  ┌────────┐ │
│  │  Content   │  │   Tools    │  │Prompts │ │
│  │  Loader    │  │            │  │        │ │
│  └─────┬──────┘  └─────┬──────┘  └───┬────┘ │
│        │               │             │      │
│        └───────────────┼─────────────┘      │
│                        │                    │
│              ┌─────────▼─────────┐          │
│              │   MCP Protocol    │          │
│              │  (stdio/HTTP)     │          │
│              └───────────────────┘          │
└─────────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────┐
│              pmcp-course/src/                │
│  ├── part1-foundations/                      │
│  │   ├── ch01-enterprise-case.md            │
│  │   └── ...                                │
│  └── quizzes/                               │
│      ├── ch02-01-setup.toml                 │
│      └── ...                                │
└─────────────────────────────────────────────┘
```

## Running

### Prerequisites

```bash
# From the rust-mcp-sdk root
cd examples/27-course-server-minimal
```

### With stdio transport

```bash
# Uses pmcp-course content from repository
CONTENT_DIR=../../pmcp-course/src cargo run

# Or with custom content directory
CONTENT_DIR=/path/to/your/course cargo run
```

### Test with MCP Inspector

```bash
# In one terminal, run the server
CONTENT_DIR=../../pmcp-course/src cargo run

# In another terminal, connect with inspector
npx @anthropic-ai/mcp-inspector stdio cargo run --manifest-path examples/27-course-server-minimal/Cargo.toml
```

## MCP Interface

### Tools

| Tool | Description | Input |
|------|-------------|-------|
| `list_chapters` | List all chapters in the course | `{}` |
| `get_lesson` | Get a specific chapter by ID | `{ "chapter_id": "ch01-enterprise-case" }` |
| `get_quiz` | Get quiz questions (no answers) | `{ "quiz_id": "ch02-01-setup" }` |

### Resources

| URI Pattern | Description |
|-------------|-------------|
| `course://chapters/{chapter_id}` | Chapter markdown content |

### Prompts

| Prompt | Description | Arguments |
|--------|-------------|-----------|
| `start_learning` | Begin learning journey | None |
| `review_chapter` | Review a specific chapter | `chapter_id` (required) |

## Example Interactions

### List Chapters

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "list_chapters",
    "arguments": {}
  }
}

// Response
{
  "chapters": [
    {
      "id": "ch01-enterprise-case",
      "title": "The Enterprise Case for MCP",
      "section_count": 0,
      "has_quiz": false
    },
    {
      "id": "ch02-first-server",
      "title": "Your First MCP Server",
      "section_count": 0,
      "has_quiz": true
    }
  ],
  "total": 2
}
```

### Get Lesson

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "get_lesson",
    "arguments": {
      "chapter_id": "ch01-enterprise-case"
    }
  }
}

// Response
{
  "chapter_id": "ch01-enterprise-case",
  "title": "The Enterprise Case for MCP",
  "content": "# The Enterprise Case for MCP\n\n...",
  "sections": [],
  "has_quiz": false
}
```

### Read Chapter Resource

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "resources/read",
  "params": {
    "uri": "course://chapters/ch01-enterprise-case"
  }
}

// Response
{
  "contents": [
    {
      "uri": "course://chapters/ch01-enterprise-case",
      "mimeType": "text/markdown",
      "text": "# The Enterprise Case for MCP\n\n..."
    }
  ]
}
```

## What This Example Demonstrates

### Content Loading
- Parsing directory structure for chapters
- Loading TOML quiz files
- Extracting metadata from markdown

### MCP Resources
- URI template patterns (`course://chapters/{chapter_id}`)
- Resource listing and reading
- MIME type handling

### MCP Tools
- Input validation with schemars
- JSON response formatting
- Error handling

### MCP Prompts
- Static and parameterized prompts
- Content injection into prompts

## What's NOT Included (See Full Version)

This minimal example omits:
- **OAuth authentication** - No user identification
- **Progress tracking** - No state persistence
- **Quiz validation** - Answers not checked server-side
- **Prerequisite enforcement** - All content accessible
- **Achievement system** - No gamification
- **HTTP transport** - Stdio only

For these features, see the [full course server](https://github.com/your-org/pmcp-run/tree/main/built-in/mdbook-course).

## Code Structure

```
src/
└── main.rs          # All-in-one implementation (~350 lines)
    ├── Data types   # Chapter, Quiz, Section structs
    ├── Content loading  # Filesystem parsing
    ├── Tool handlers    # get_lesson, list_chapters, get_quiz
    ├── Resource handlers  # Chapter content
    └── Prompt handlers    # Learning prompts
```

## Extending This Example

### Add HTTP Transport

```rust
// Replace stdio with HTTP
use pmcp::server::http::HttpServer;

HttpServer::new(mcp_server)
    .bind("0.0.0.0:3000")
    .serve()
    .await?;
```

### Add Progress Tracking

```rust
struct ProgressStore {
    completed: HashSet<String>,
}

// Track in tool handlers
fn get_lesson(&self, input: GetLessonInput) -> Result<GetLessonOutput> {
    self.progress.mark_viewed(&input.chapter_id);
    // ...
}
```

### Add Quiz Validation

```rust
fn submit_answer(&self, input: SubmitAnswerInput) -> Result<SubmitAnswerOutput> {
    let quiz = self.content.quizzes.get(&input.quiz_id)?;
    let question = quiz.get_question(&input.question_id)?;

    // Never send correct answer to client
    let is_correct = question.validate(&input.answer);

    Ok(SubmitAnswerOutput {
        correct: is_correct,
        explanation: question.context.clone(),
        // ...
    })
}
```

## Related

- [PMCP Advanced Course](../../pmcp-course/) - The course content
- [Full Course Server Design](https://github.com/your-org/pmcp-run/tree/main/built-in/mdbook-course/DESIGN.md)
- [Quiz Format Specification](https://github.com/your-org/pmcp-run/tree/main/built-in/mdbook-course/QUIZ_FORMAT.md)
