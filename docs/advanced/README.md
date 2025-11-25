# Advanced Guides

This directory contains advanced guides and patterns for building production-ready MCP servers with the Rust SDK.

## Table of Contents

### User Interface & Visualization

- **[MCP Apps Extension](mcp-apps-extension.md)** - Build interactive UIs with maps, galleries, charts, and dashboards
  - Interactive map example with Leaflet.js
  - Image gallery with lightbox
  - Real-time data visualization
  - Security best practices

### Architecture & Patterns

- **[Middleware Composition](middleware-composition.md)** - Advanced middleware patterns and composition strategies
  - Authentication middleware
  - Rate limiting
  - Request/response transformation
  - Error handling

- **[Session Management](session-management.md)** - Managing stateful sessions and user contexts
  - Session lifecycle
  - State persistence
  - Multi-user scenarios
  - Session security

### Deployment & Operations

- **[Production Deployment](production-deployment.md)** - Deploy and operate MCP servers in production
  - Infrastructure setup
  - Monitoring and observability
  - Performance optimization
  - High availability

### Migration & Integration

- **[Migration from TypeScript](migration-from-typescript.md)** - Migrate existing TypeScript MCP servers to Rust
  - API mapping
  - Type conversion
  - Common patterns
  - Performance considerations

## Quick Links

### Getting Started
- [Main Documentation](../README.md)
- [TypedTool Guide](../TYPED_TOOLS_GUIDE.md)
- [Testing Guide](../COMPREHENSIVE_TESTING_GUIDE.md)

### Examples
- [Conference Venue Map](../../examples/conference_venue_map.rs)
- [Hotel Gallery](../../examples/hotel_gallery.rs)
- [All Examples](../../examples/)

## Contributing

Found an issue or want to improve these guides? Please contribute!

1. Check existing [documentation](../)
2. Review [examples](../../examples/)
3. Submit improvements via pull request
