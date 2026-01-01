# Prerequisites

Welcome! This course is designed to be accessible to enterprise developers coming from any background. Whether you're a Java architect, C# backend developer, or Python data engineer, you'll find familiar concepts here—just expressed in Rust's syntax.

## Our Learning Philosophy: Read, Don't Write

**You need to know how to read Rust code, not how to write it.**

This course provides extensive code examples that you'll read to understand concepts. When it comes to writing code, you'll use AI coding assistants (Claude Code, Cursor, Copilot) to do the heavy lifting. Your job is to:

1. **Understand** what the code is doing
2. **Instruct** the AI what you want to build
3. **Review** the generated code
4. **Run** the compiler to catch any issues

The Rust compiler becomes your safety net—if it compiles, it almost certainly works correctly. This is why Rust is uniquely suited for AI-assisted development.

## Why This Approach Works

Rust has an exceptional compiler that provides clear, actionable error messages. Combined with AI assistants that can read and fix these errors, you get a powerful feedback loop:

```
You describe what you want
    ↓
AI generates Rust code
    ↓
Compiler catches issues (if any)
    ↓
AI fixes issues automatically
    ↓
Working, production-ready code
```

We cover this in depth in [Part VI: AI-Assisted Development](./part6-ai-dev/ch15-ai-assisted.md), where you'll learn how to effectively collaborate with AI assistants to build MCP servers.

## Rust Concepts You'll Encounter

Don't worry if these aren't familiar yet—you'll learn them through the code examples.

### Familiar Concepts (Coming from Java/C#)

| Java/C# | Rust | Example |
|---------|------|---------|
| `class` | `struct` | `struct User { name: String }` |
| `interface` | `trait` | `trait Tool { fn call(&self); }` |
| `try/catch` | `Result<T, E>` | `Ok(value)` or `Err(error)` |
| `nullable` | `Option<T>` | `Some(value)` or `None` |
| `async/await` | `async/await` | Same concept, same keywords! |
| Generics `<T>` | Generics `<T>` | Same syntax! |

### Rust-Specific Concepts

You'll see these in code examples. AI assistants handle them well:

- **Ownership & borrowing** - Rust's way of managing memory without garbage collection. The compiler ensures you use references safely. You'll see `&` and `&mut` in function signatures.

- **The `?` operator** - A clean way to propagate errors. When you see `result?`, it means "return the error if there is one, otherwise continue."

- **Pattern matching** - Like a powerful `switch` statement. You'll see `match` and `if let` used to handle `Result` and `Option` values.

- **Macros** - Code that generates code. You'll see `#[derive(...)]` annotations that automatically implement common functionality.

### What You Don't Need to Master

These advanced topics are handled by AI assistants and the PMCP SDK:

- Lifetime annotations (`'a`, `'static`)
- Unsafe Rust
- Advanced trait bounds
- Macro writing
- Memory layout optimization

## Technical Prerequisites

### Required Tools

```bash
# You'll set these up in Chapter 2
rust (latest stable)    # Programming language
cargo-pmcp              # MCP development toolkit
```

### Helpful Background

**HTTP and APIs** (you probably already know this):
- HTTP methods (GET, POST)
- JSON format
- REST API concepts

**Command Line** (basic comfort):
- Running commands
- Environment variables

### Cloud Platforms (For Deployment Chapters)

Parts III-V cover deployment. Familiarity with one is helpful:
- **AWS** - Lambda, API Gateway
- **Cloudflare** - Workers
- **Google Cloud** - Cloud Run

Don't worry if cloud is new—we guide you step by step.

## Environment Setup

Chapter 2 includes an interactive setup exercise that guides you through:

- Installing Rust
- Installing cargo-pmcp
- Configuring your MCP client (Claude Desktop, VS Code, etc.)

**[Go to Environment Setup Exercise →](./part1-foundations/ch02-ex00-setup.md)**

## A Note for Enterprise Developers

If you're coming from enterprise Java or C#, you'll find that:

1. **Rust's type system** is similar to what you know, with some additions for safety
2. **The package manager (Cargo)** is more ergonomic than Maven or NuGet
3. **Error handling** uses explicit types instead of exceptions—cleaner once you're used to it
4. **No null pointer exceptions** ever—Rust simply doesn't have null

The strictness that might seem unusual at first is exactly what makes Rust reliable for enterprise systems. And with AI assistants handling the syntax, you can focus on the architecture and business logic you're already expert in.

## Ready to Start?

You're ready if you can:

- [ ] Read code and understand its intent
- [ ] Describe what you want to build in plain English
- [ ] Run commands in a terminal
- [ ] Accept that AI will write most of your code

That's it. The compiler and AI handle the rest.

---

*Continue to [Part I: Foundations](./part1-foundations/ch01-enterprise-case.md) →*
