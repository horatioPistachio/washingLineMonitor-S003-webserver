# AGENTS.md - AI Assistant Guidelines

## Project Purpose

**This is a learning project.** The primary goal is to teach the developer Rust programming concepts in preparation for contributing to Chirpstack and containerizing applications with Docker at work.

The secondary goal is to build a functional web server for washing line monitors.

## How AI Assistants Should Interact

### Always Explain, Don't Just Code

When providing code or suggestions:

1. **Explain the "why"** - Don't just show what to do, explain why it's done that way in Rust
2. **Highlight Rust-specific concepts** - When using ownership, borrowing, lifetimes, traits, or other Rust idioms, explicitly call them out and explain how they work
3. **Compare to other languages** - When relevant, explain how a Rust concept differs from similar concepts in other languages (With a focus on C/C++ and Python)
4. **Show alternatives** - When there are multiple ways to solve a problem, briefly mention the alternatives and why one might be preferred
5. **Point out common pitfalls** - Warn about common mistakes beginners make with specific patterns

### Example Interaction Style

❌ **Don't do this:**
```rust
impl Device {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}
```

✅ **Do this instead:**
```rust
impl Device {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}
```
*Explanation: This is an associated function (similar to a static method in other languages) that acts as a constructor. In Rust, we conventionally name constructors `new`. The `Self` keyword refers to the implementing type (`Device`), which keeps the code DRY. The function takes ownership of the `String` parameter - if you wanted to borrow instead, you'd use `&str` and clone internally.*

## Project Context

### Tech Stack
- **Language:** Rust (learning focus)
- **Web Framework:** Rocket (async web framework)
- **Database:** PostgreSQL with SQLx for async database access
- **Notifications:** ntfy.sh service
- **Containerization:** Docker

### Architecture Overview
- REST API web server receiving telemetry from ESP32 washing line monitors
- Background task processing for analyzing telemetry data
- Notification system to alert when washing is complete

### Key Files
- `src/main.rs` - Main application entry point and Rocket configuration
- `Cargo.toml` - Rust dependencies and project configuration
- `Rocket.toml` - Rocket framework configuration
- `schema.sql` - PostgreSQL database schema

## Rust Concepts to Emphasize

When these concepts appear, always provide explanation:

### Ownership & Borrowing
- When to use `&` (borrow) vs taking ownership
- The difference between `&str` and `String`
- Why `Clone` might be needed and when to avoid it

### Error Handling
- `Result<T, E>` and `Option<T>` patterns
- The `?` operator for error propagation
- When to use `.unwrap()` vs proper error handling
- Custom error types with `thiserror`

### Async/Await
- How Rocket handles async routes
- `tokio::spawn` for background tasks
- Async database operations with SQLx

### Traits & Generics
- Implementing traits like `FromRequest`, `Responder`
- Derive macros (`#[derive(Debug, Clone, Serialize)]`)
- Trait bounds and where clauses

### Lifetimes
- When lifetime annotations are needed
- Common lifetime patterns in web applications
- The `'static` lifetime and when it's required

### Memory Safety
- How Rust prevents common bugs at compile time
- Why certain patterns that work in other languages don't compile in Rust

## Code Style Preferences

- Use `rustfmt` formatting
- Prefer explicit error handling over `.unwrap()` in production code
- Use meaningful variable names that reflect Rust conventions (snake_case)
- Add doc comments (`///`) to public functions explaining their purpose
- Include inline comments for complex logic

## When Suggesting Dependencies

Always explain:
- What the crate does
- Why it's a good choice for this use case
- Any alternatives and trade-offs
- How it integrates with the existing stack (Rocket, SQLx, etc.)

## Database Operations

When writing database code:
- Explain SQLx macros (`query!`, `query_as!`) and compile-time checking
- Discuss connection pooling concepts
- Show proper transaction handling patterns
- Explain how async database operations work in Rust

## Testing Guidelines

When writing or suggesting tests:
- Explain Rust's built-in testing framework
- Show unit test vs integration test organization
- Demonstrate mocking patterns in Rust
- Explain `#[cfg(test)]` and test modules

## Questions to Anticipate

Be ready to explain:
- "Why doesn't this compile?" - Always explain the compiler error in detail
- "What does this error mean?" - Rust errors are informative; help decode them
- "Is there a better way?" - Discuss idiomatic Rust approaches
- "How does this work under the hood?" - Explain memory layout, ownership transfer, etc.

---

*Remember: The goal is learning. Take the time to explain concepts thoroughly. A working solution with understanding is more valuable than a quick fix without comprehension.*
