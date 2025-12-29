# Chapter 5 Exercises

These exercises will help you implement robust validation with AI-friendly error messages.

## Quiz

Test your understanding of validation and schema design:

{{#quiz ../quizzes/ch05-validation.toml}}

## Exercises

1. **[Validation Errors for AI](./ch05-ex01-validation-errors.md)** ⭐⭐ Intermediate (25 min)
   - Implement a ValidationError struct with helpful fields
   - Create errors that help AI clients self-correct
   - Apply the four levels of validation

## Key Concepts to Practice

- **The Feedback Loop**: Errors are how AI learns to use your tools correctly
- **Structured Error Codes**: RATE_LIMITED, NOT_FOUND, INVALID_FORMAT enable programmatic decisions
- **Expected vs Received**: Always show what you expected and what was sent
- **Examples in Errors**: Include concrete examples the AI can copy

## Next Steps

After completing these exercises, continue to:
- [Resources, Prompts, and Workflows](./ch06-beyond-tools.md) - Give users control
- [Chapter 4 Exercises](./ch04-exercises.md) - Review tool design principles
