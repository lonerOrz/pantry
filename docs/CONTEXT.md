# Pantry Domain Model

## Glossary

### Preview
The process of generating a displayable representation (image or text) for a selected item.

### PreviewStrategy
A specific implementation for generating a `PreviewPayload` based on an `Item`'s properties (type, display mode, source).

### Orchestrator
The `PreviewService` that selects and executes the appropriate `PreviewStrategy` for an item.

## Decisions

### ADR-002: Preview Strategy Pattern
Decided to extract preview generation logic into a `PreviewStrategy` trait to decouple the `PreviewService` orchestration logic from specific media decoding/generation implementations.
