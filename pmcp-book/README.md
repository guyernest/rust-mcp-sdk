# The PMCP Guide 📚

**Comprehensive documentation for the PMCP (Pragmatic Model Context Protocol) Rust SDK**

## 🎯 About This Book

This book provides comprehensive documentation for PMCP, following the same approach as the successful ruchy-book. It includes:

- **Implementation-first documentation** - All examples are tested and working
- **Progressive learning path** - From basics to advanced topics  
- **Interactive examples** - Runnable code with quality indicators
- **Toyota Way standards** - Zero tolerance for defects in documentation
- **TypeScript SDK compatibility** - Complete migration guides and feature parity

## 🚀 Quick Start

### Building the Book

```bash
# Build the book
make book

# Serve locally with live reload
make book-serve

# Test all examples
make book-test

# Build and open in browser
make book-open
```

### Mermaid Diagram Support

This book renders Mermaid diagrams via the `mdbook-mermaid` preprocessor. Install it once:

```bash
cargo install mdbook-mermaid
```

Then build/serve as usual (`mdbook build` or `mdbook serve`). If the plugin is not installed, Mermaid code blocks will not render in HTML.

Troubleshooting:
- Ensure the plugin is in your PATH: `export PATH="$HOME/.cargo/bin:$PATH"`
- Verify install: `mdbook-mermaid --version`
- Check build logs for a missing preprocessor warning; if seen, reinstall the plugin and restart `mdbook serve`.
- Diagrams still low-contrast? We force a neutral Mermaid theme and add CSS for better readability in light/dark modes.

### Manual Commands

```bash
# Install mdBook if needed
cargo install mdbook

# Build the book
cd pmcp-book
mdbook build

# Serve with live reload
mdbook serve --open
```

## 📖 Content Structure

### Part I: Getting Started
- Installation & Setup
- Your First MCP Server  
- Your First MCP Client
- Understanding the Protocol

### Part II: Core Concepts
- Tools & Tool Handlers
- Resources & Resource Management
- Prompts & Templates
- Error Handling & Recovery

### Part III: Advanced Features
- Authentication & Security
- Transport Layers (WebSocket, HTTP, Streaming)
- Middleware & Composition
- Progress Tracking & Cancellation

### Part IV: Real-World Applications
- Building Production Servers
- Performance & Optimization
- Testing & Quality Assurance
- Deployment Strategies

### Part V: Examples & Patterns
- Complete working examples
- Design patterns and best practices
- Integration patterns

### Part VI: TypeScript SDK Compatibility
- Interoperability guides
- Migration from TypeScript
- Feature parity documentation

### Part VII: Advanced Topics
- Custom transports
- Protocol extensions
- Performance analysis
- Contributing guidelines

## 🎨 Features

### Custom Theme
- **PMCP Branding** - Custom colors and styling
- **Code Enhancement** - Syntax highlighting for Rust and PMCP-specific tokens
- **Interactive Elements** - Copy buttons, expand/collapse, run buttons
- **Quality Indicators** - Visual indicators for code quality and testing status

### Interactive Examples
- **Quality Badges** - Complexity and quality indicators
- **Runnable Code** - Integration with playground (planned)
- **Test Integration** - Inline test execution
- **Copy-Paste Ready** - All examples are complete and working

### Documentation Quality
- **Test-Driven Documentation** - All examples tested before inclusion
- **Toyota Way Standards** - Zero tolerance for defects
- **Comprehensive Coverage** - From basics to advanced topics
- **Real-World Focus** - Production-ready patterns and practices

## 🛠️ Development

### Adding New Chapters

1. Add chapter to `src/SUMMARY.md`
2. Create markdown file in `src/`
3. Include working code examples
4. Add tests for code examples
5. Build and test: `make book-test`

### Theme Customization

Theme files are in `theme/`:
- `pmcp.css` - Main PMCP styling
- `code-enhancements.css` - Code block enhancements
- `syntax-highlight.css` - PMCP-specific syntax highlighting
- `pmcp-highlight.js` - Interactive highlighting
- `examples.js` - Example interactivity

### Testing Examples

All code examples should be:
- Complete and runnable
- Include proper error handling
- Follow PMCP best practices
- Pass quality gates (lint, format, test)

## 📊 Quality Standards

### Code Quality
- ✅ All examples tested and working
- ✅ Zero clippy warnings
- ✅ Proper error handling
- ✅ Documentation comments
- ✅ Toyota Way compliance

### Documentation Quality  
- ✅ Progressive difficulty curve
- ✅ Clear explanations
- ✅ Working examples
- ✅ Real-world applicability
- ✅ Cross-references and links

## 🔗 Integration

The book integrates with the main PMCP project:

- **Examples sync** - Examples mirror the main examples/ directory
- **Version tracking** - Automatically tracks PMCP version compatibility
- **CI integration** - Built and tested in CI/CD pipeline
- **Quality gates** - Same standards as main codebase

## 📈 Roadmap

### Current (v1.0)
- ✅ Basic structure and theme
- ✅ Core chapters (Getting Started, Core Concepts)
- ✅ Interactive examples
- ✅ Quality indicators

### Planned (v1.1)
- [ ] Complete all chapter content
- [ ] Playground integration
- [ ] Multi-language support
- [ ] PDF export
- [ ] Search optimization

### Future (v2.0)  
- [ ] Interactive tutorials
- [ ] Video content integration
- [ ] Community contributions
- [ ] Advanced theming

## 🤝 Contributing

Contributions welcome! Please:

1. Follow Toyota Way quality standards
2. Test all code examples
3. Maintain consistent styling
4. Update table of contents
5. Run quality gates: `make book-test`

## 📝 License

MIT License - same as PMCP main project.

---

**Happy reading! 📖✨**
