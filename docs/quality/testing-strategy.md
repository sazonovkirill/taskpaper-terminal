# Testing Strategy

- Status: Accepted
- Last updated: 2026-10-04

## Owner requirements

These are the owner's requirements. Everything else in this document serves them.

1. **Full and complete coverage, within reason.** All owned production behavior is tested. Exclusions are narrow, explicit and justified.
2. **Clear separation between unit tests and integration (UI) tests.** The two kinds live in different places, run as separately selectable suites and are never mixed in one test.
3. **Every type and standalone function has a complete set of unit tests** covering the happy path, error cases and edge cases.
4. **Tests verify the contract, not the implementation.** A test states what a unit promises to its callers through its public interface. Refactoring that keeps the contract must not require rewriting tests.
5. **Integration (UI) tests cover every user interaction with the app, within reason.** Every key binding, command, mode and screen a user can reach has at least one test driving it through the app's input handling and checking what the user sees and what is saved.
6. **Every test can run in parallel** with every other test.
