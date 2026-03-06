# NiXQueryLink360

**Universal SQL Gateway** — A high-performance, Rust-based alternative to Databricks SQL Endpoint that speaks the Databricks Statement Execution API v2.0, but can execute queries against any backend: Databricks warehouses, Amazon S3, Azure Blob Storage, Delta Lake, Apache Iceberg, and more.

---

## Vision

Databricks SQL Endpoint is a great interface — clients already know how to talk to it (REST, JDBC, ODBC).
But the backend doesn't have to be Databricks.

NiXQueryLink360 decouples the **wire protocol** (Databricks SQL API) from the **query engine** and **storage layer**, letting you point the same client tooling at any data source:

```
Client (JDBC / ODBC / REST / BI tools)
               │
               │  Databricks SQL Statement Execution API v2.0
               ▼
  ┌────────────────────────────────────────┐
  │         NiXQueryLink360                │
  │         Universal SQL Gateway          │
  │                                        │
  │  ┌──────────────────────────────────┐  │
  │  │         Query Router             │  │
  │  │  (warehouse_id → backend)        │  │
  │  └──────────┬───────────────────────┘  │
  │             │                          │
  │    ┌────────┴──────────┐               │
  │    ▼                   ▼               │
  │  ┌──────────┐  ┌────────────────────┐  │
  │  │Databricks│  │  DataFusion Engine │  │
  │  │  Proxy   │  │  (Embedded Rust)   │  │
  │  │(Phase 1) │  │  (Phase 2)         │  │
  │  └──────────┘  └─────────┬──────────┘  │
  └────────────────────────┬─┼─────────────┘
                           │ │
            ┌──────────────┘ └───────────────────┐
            ▼                                    ▼
  Databricks SQL Warehouse          Object Storage
  (Statement Execution API)    ┌────┴────────────────┐
                               │  S3 / Azure Blob / GCS
                               │  ├── Parquet
                               │  ├── Delta Lake
                               │  ├── Apache Iceberg
                               │  └── CSV / JSON / ORC
                               └────────────────────────
```

---

## Key Features

| Feature | Status |
|---------|--------|
| Databricks SQL Statement Execution API v2.0 compatible | ✅ Phase 1 |
| Proxy mode — forward queries to Databricks warehouse | ✅ Phase 1 |
| Bearer token auth + per-warehouse token injection | ✅ Phase 1 |
| Structured logging + request correlation IDs | ✅ Phase 1 |
| Exponential backoff retry | ✅ Phase 1 |
| Embedded DataFusion query engine | 🔜 Phase 2 |
| S3 / Azure Blob / GCS object storage backend | 🔜 Phase 2 |
| Delta Lake table format | 🔜 Phase 2 |
| Apache Iceberg table format | 🔜 Phase 2 |
| PostgreSQL wire protocol (JDBC / ODBC direct) | 🔜 Phase 3 |
| Query result caching | 🔜 Phase 4 |
| Unity Catalog-compatible metadata layer | 🔜 Phase 4 |

---

## Architecture

NiXQueryLink360 follows **Clean Architecture** with strict layer separation:

```
interfaces/   HTTP handlers, middleware, DTOs
application/  Use cases (submit, get, cancel)
domain/       Entities, ports, errors (no external deps)
infrastructure/  Databricks client, DataFusion engine, config
```

Each backend (Databricks, DataFusion, …) implements the same `WarehouseClient` port trait. The router selects the correct backend based on `warehouse_id` at runtime — zero code changes needed in the HTTP or application layers.

---

## Phases

See [ROADMAP.md](ROADMAP.md) for the full roadmap.

| Phase | Scope | Status |
|-------|-------|--------|
| [Phase 1](PHASE1_REQUIREMENTS.md) | Databricks proxy foundation | 🟡 In Progress |
| [Phase 2](PHASE2_REQUIREMENTS.md) | DataFusion engine + object storage | 📋 Planned |
| Phase 3 | PostgreSQL wire protocol (JDBC/ODBC) | 📋 Planned |
| Phase 4 | Caching, metadata layer, Unity Catalog compat | 📋 Planned |

---

## Quick Start

```bash
# Copy and fill in your config
cp config.toml.example config.toml

# Run
cargo run --release

# Health check
curl http://localhost:8360/health

# Submit a SQL statement (Databricks backend)
curl -X POST http://localhost:8360/api/2.0/sql/statements \
  -H "Authorization: Bearer <your-token>" \
  -H "Content-Type: application/json" \
  -d '{
    "statement": "SELECT 1 AS n",
    "warehouse_id": "wh-prod",
    "wait_timeout": "10s"
  }'
```

---

## Tech Stack

| Component | Crate | Notes |
|-----------|-------|-------|
| Async runtime | `tokio` | |
| HTTP server | `axum 0.8` | |
| HTTP client | `reqwest 0.12` | Databricks proxy backend |
| Query engine | `datafusion` | Phase 2 — embedded SQL engine |
| Object storage | `object_store` | S3, Azure Blob, GCS |
| Delta Lake | `deltalake` | Phase 2 |
| Iceberg | `iceberg` | Phase 2 |
| Serialization | `serde` + `serde_json` | |
| Config | `config 0.15` | TOML + env vars |
| Logging | `tracing` + `tracing-subscriber` | |
| Error handling | `thiserror` + `anyhow` | |

---

*Built with Rust 🦀 | Apache-2.0 License*
