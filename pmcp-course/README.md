# Advanced MCP: Enterprise-Grade AI Integration with Rust

An advanced course on building production-ready MCP servers using the PMCP SDK and cargo-pmcp toolkit.

## Target Audience

- **Developers** building MCP servers for enterprise deployments
- **Technical leads** evaluating MCP for their organizations
- **Business stakeholders** wanting to understand the value proposition

## Course Modules

### Part I: Foundations (Start Here)
Building your first production-ready servers—calculator and database access.

### Part II: Thoughtful Design
Moving beyond "tool sprawl" to cohesive, well-designed APIs.

### Part III: Cloud Deployment
Deploy to AWS Lambda, Cloudflare Workers, and Google Cloud Run.

### Part IV: Testing
Schema-driven test generation and CI/CD integration.

### Part V: Enterprise Security
OAuth authentication with Cognito, Auth0, and Entra ID.

### Part VI: AI-Assisted Development
Using Claude Code and other AI assistants for development.

### Part VII: Observability
Middleware for logging, metrics, and pmcp.run integration.

### Part VIII: Advanced Patterns
Server composition, MCP applications, and high availability.

## Building the Course

```bash
cd pmcp-course
mdbook serve --open
```

## Prerequisites

See [src/prerequisites.md](src/prerequisites.md) for detailed setup instructions.

## Course Structure

```
pmcp-course/
├── book.toml           # mdBook configuration
├── README.md           # This file
└── src/
    ├── SUMMARY.md      # Table of contents
    ├── introduction.md # Course introduction
    ├── prerequisites.md
    ├── part1-foundations/
    │   ├── ch01-enterprise-case.md
    │   ├── ch02-first-server.md
    │   └── ch03-database-servers.md
    ├── part2-design/
    ├── part3-deployment/
    ├── part4-testing/
    ├── part5-security/
    ├── part6-ai-dev/
    ├── part7-observability/
    ├── part8-advanced/
    └── appendix/
```

## Contributing

This course is part of the [PMCP SDK](https://github.com/paiml/rust-mcp-sdk) project.

## License

MIT
