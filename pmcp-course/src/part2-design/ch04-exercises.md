# Chapter 4 Exercises

These exercises will help you practice designing cohesive, well-structured MCP tool sets.

## Quiz

Test your understanding of the design principles covered in this chapter:

{{#quiz ../quizzes/ch04-design-principles.toml}}

## Exercises

1. **[Tool Design Review](./ch04-ex01-tool-design-review.md)** ⭐⭐ Intermediate (30 min)
   - Review a poorly designed MCP server
   - Identify anti-patterns and propose improvements
   - Apply domain prefixing and single responsibility

## Key Concepts to Practice

- **Domain Prefixing**: Use `sales_`, `customer_`, `order_` prefixes to avoid collisions
- **Single Responsibility**: Each tool does one thing well
- **The 50 Tools Test**: Would your tools be distinguishable in a crowded environment?
- **The One Sentence Rule**: Can you describe each tool in one clear sentence?

## Next Steps

After completing these exercises, continue to:
- [Input Validation and Output Schemas](./ch05-validation.md) - Make your tools AI-friendly
- [Resources, Prompts, and Workflows](./ch06-beyond-tools.md) - Beyond just tools
