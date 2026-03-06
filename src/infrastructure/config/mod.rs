//! Configuration infrastructure — loads and validates runtime settings.
//!
//! Settings are sourced from (in priority order):
//! 1. Environment variables prefixed with `NQL__` (highest priority)
//! 2. `config.toml` in the working directory (if present)
//! 3. Built-in defaults (lowest priority)
//!
//! See [`settings::Settings::load`] for details.

pub mod settings;
