//! Application layer — orchestrates use cases without containing business rules.
//!
//! Each use case receives input from the interface layer, validates it, and
//! delegates to the domain and infrastructure layers via port traits.  Use
//! cases are the only place where cross-cutting concerns such as input
//! validation, transaction boundaries, and structured logging are handled.
//!
//! The application layer depends on `domain` (entities + ports) but **never**
//! on `infrastructure` or `interfaces` directly.

pub mod use_cases;
