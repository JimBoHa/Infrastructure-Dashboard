Refactor outputs.rs into Smaller Modules

  Priority: P2 (Maintainability)
  Status: To Do
  Estimated Effort: Medium (4-6 hours)

  Problem

  apps/core-server-rs/src/routes/outputs.rs is 783 lines - the largest route file. It handles:

  1. Output CRUD operations (create, read, update, delete)
  2. Output type definitions and validation
  3. State management and history tracking
  4. MQTT command publishing
  5. Schedule association logic

  Solution

  Split into focused modules:

  routes/
  ├── outputs/
  │   ├── mod.rs          (~100 lines) - Router + re-exports
  │   ├── types.rs        (~150 lines) - OutputRow, OutputResponse, requests
  │   ├── handlers.rs     (~300 lines) - CRUD route handlers
  │   ├── validation.rs   (~100 lines) - Type/state validation
  │   └── commands.rs     (~100 lines) - MQTT command publishing

  Acceptance Criteria

  - Create routes/outputs/ module directory
  - Extract types to types.rs
  - Extract handlers to handlers.rs
  - Extract validation logic to validation.rs
  - Extract MQTT publishing to commands.rs
  - Update routes/mod.rs to use new structure
  - Verify all tests pass
  - No functionality changes

