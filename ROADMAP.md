# NiXQueryLink360 — Product Roadmap

**Last Updated:** 2026-03-06

---

## Vision

> **NiXQueryLink360 เป็น Universal SQL Gateway** ที่ expose Databricks SQL Statement Execution API v2.0 แต่ข้างหลังสามารถ execute query ได้จากหลาย backend — Databricks, S3, Azure Blob, GCS, Delta Lake, Iceberg และอื่นๆ

Client ทุกคนที่ใช้ Databricks SQL API อยู่แล้ว (JDBC, ODBC, REST, BI tools) สามารถ point มาที่ NiXQueryLink360 แทนได้ทันที โดยไม่ต้องเปลี่ยนโค้ดหรือ driver ฝั่ง client เลย

---

## Phase Overview

```
Phase 1 ──────────────────────────────────────────────────────────────────────
Foundation: HTTP server + Databricks proxy backend

  Client ──▶ NiXQueryLink360 ──▶ Databricks SQL Warehouse

Phase 2 ──────────────────────────────────────────────────────────────────────
Embedded query engine: DataFusion + Object Storage backends

  Client ──▶ NiXQueryLink360 ──▶ Databricks SQL Warehouse
                              └──▶ DataFusion Engine ──▶ S3 / Azure Blob / GCS
                                                     ├── Parquet
                                                     ├── Delta Lake
                                                     └── Apache Iceberg

Phase 3 ──────────────────────────────────────────────────────────────────────
JDBC/ODBC native support via PostgreSQL wire protocol

  Client (JDBC Driver) ──▶ NiXQueryLink360 (pgwire) ──▶ Any Phase 2 backend

Phase 4 ──────────────────────────────────────────────────────────────────────
Intelligence layer: caching, metadata, catalog, query optimization

  Client ──▶ NiXQueryLink360 ──▶ Smart Cache ──▶ Any backend
                              └──▶ Unity Catalog-compatible metadata API
```

---

## Phase 1 — Foundation & Databricks Proxy

**Status:** 🟡 In Progress
**Requirements:** [PHASE1_REQUIREMENTS.md](PHASE1_REQUIREMENTS.md)

### Scope
- HTTP server (Axum 0.8) ที่ implement Databricks SQL Statement Execution API v2.0
- Proxy mode: forward SQL queries ไปยัง Databricks SQL Warehouse
- Bearer token auth + per-warehouse token injection
- Connection pooling + exponential backoff retry
- Structured logging + request correlation IDs
- Clean Architecture foundation (`WarehouseClient` port trait) พร้อม extend Phase 2

### Key Deliverables
| Item | Status |
|------|--------|
| Axum HTTP server + all 3 SQL endpoints | ✅ |
| `DatabricksClient` impl `WarehouseClient` | ✅ |
| Bearer auth middleware | ✅ |
| Multi-warehouse routing via config | ✅ |
| Retry + exponential backoff | ✅ |
| Structured logging (tracing) | ✅ |
| Unit tests (60%+ coverage) | ✅ |
| Config (TOML + env vars) | ✅ |
| Professional doc comments | ✅ |
| Docker image | 🔜 |
| Integration tests (mock upstream) | 🔜 |

### Estimated Duration
~3–4 สัปดาห์

---

## Phase 2 — DataFusion Engine + Object Storage

**Status:** 📋 Planned
**Requirements:** [PHASE2_REQUIREMENTS.md](PHASE2_REQUIREMENTS.md)

### Scope
- Embed **Apache DataFusion** เป็น in-process SQL query engine
- รองรับ **AWS S3**, **Azure Blob Storage**, **GCS** ผ่าน `object_store` crate
- Table formats: **Apache Parquet**, **Delta Lake** (delta-rs), **Apache Iceberg**
- Multi-backend router — `warehouse_id` ชี้ไป Databricks หรือ DataFusion ก็ได้
- In-memory statement store สำหรับ async polling ของ DataFusion queries
- ARROW_STREAM result format

### Key Deliverables
| Item | Backend | Status |
|------|---------|--------|
| `DataFusionClient` impl `WarehouseClient` | DataFusion | 📋 |
| S3 object store integration | AWS S3 | 📋 |
| Azure Blob object store integration | Azure Blob | 📋 |
| Static catalog from config.toml | Both | 📋 |
| Parquet table registration + query | DataFusion | 📋 |
| Delta Lake table registration + query | DataFusion | 📋 |
| Iceberg table registration + query | DataFusion | 📋 |
| Async statement lifecycle (in-memory store) | DataFusion | 📋 |
| Backend router (warehouse_id → backend) | Both | 📋 |
| ARROW_STREAM result format | Both | 📋 |

### Tech Stack Additions
```toml
datafusion  = "44"          # Apache DataFusion (Rust query engine)
object_store = "0.11"       # S3, Azure Blob, GCS unified abstraction
deltalake   = "0.22"        # delta-rs — Delta Lake native Rust
iceberg     = "0.4"         # iceberg-rust — Apache Iceberg
```

### Estimated Duration
~4–6 สัปดาห์

---

## Phase 3 — PostgreSQL Wire Protocol (JDBC/ODBC Native)

**Status:** 💡 Concept
**Target:** Q3 2026

### Problem
Phase 1–2 รองรับ REST API เท่านั้น JDBC/ODBC driver ส่วนใหญ่ต้องการ binary protocol (Thrift สำหรับ Hive, PostgreSQL wire protocol สำหรับทั่วไป)

### Scope
- Implement **PostgreSQL wire protocol** (`pgwire` crate) เพิ่มเป็น second listener (default port: `5432`)
- รับ SQL ผ่าน PostgreSQL protocol แล้ว route ไปยัง DataFusion หรือ Databricks backend เหมือนเดิม
- รองรับ standard JDBC driver ที่ใช้ PostgreSQL dialect
- รองรับ `psql` command-line tool
- Simple query protocol + extended query protocol (prepared statements)

### Architecture Addition
```
JDBC/ODBC Client (PostgreSQL driver)
        │ Port 5432 — PostgreSQL wire protocol
        ▼
NiXQueryLink360 — pgwire listener
        │
        ▼ (reuse same application use cases)
Application Layer (submit_statement, get_statement)
        │
        ▼
WarehouseClient (Databricks | DataFusion) — unchanged
```

### Key Crate
```toml
pgwire = "0.24"   # Pure Rust PostgreSQL wire protocol server
```

---

## Phase 4 — Intelligence Layer

**Status:** 💡 Concept
**Target:** Q4 2026

### Scope

#### 4.1 Query Result Caching
- Cache ผล query ที่ identical (same SQL + same params + same table version) ใน Redis หรือ in-memory LRU
- TTL-based invalidation + Delta Lake / Iceberg snapshot-aware cache invalidation
- Config: enable/disable per-warehouse, TTL, max cache size

#### 4.2 Unity Catalog-Compatible Metadata API
- Expose metadata endpoint ที่ compatible กับ Databricks Unity Catalog REST API
- รองรับ `GET /api/2.1/unity-catalog/catalogs`, `/schemas`, `/tables`
- BI tools (Tableau, Power BI, Metabase) ที่ใช้ Unity Catalog protocol สามารถ browse tables ได้

#### 4.3 Query Federation & Optimization
- Cross-backend JOIN: `SELECT * FROM databricks.orders JOIN s3.products ON …`
- DataFusion Ballista integration สำหรับ distributed query (multi-node)
- Adaptive query planning ตาม data statistics

#### 4.4 OAuth 2.0 / OIDC Authentication
- รองรับ OAuth 2.0 token exchange
- Integration กับ Azure AD, AWS Cognito, Keycloak
- Row-level security ตาม user identity

#### 4.5 Web Dashboard
- Monitoring UI: query history, latency, cache hit rate, backend health
- Table browser ที่ integrate กับ static catalog

---

## Backend Compatibility Matrix

| Backend | Phase | Parquet | Delta Lake | Iceberg | CSV | JSON |
|---------|-------|---------|------------|---------|-----|------|
| Databricks | 1 | via Databricks | via Databricks | via Databricks | via Databricks | via Databricks |
| DataFusion + S3 | 2 | ✅ | ✅ | ✅ | ✅ | ✅ |
| DataFusion + Azure Blob | 2 | ✅ | ✅ | ✅ | ✅ | ✅ |
| DataFusion + GCS | 2 | ✅ | ✅ | ✅ | ✅ | ✅ |
| DataFusion + MinIO (S3 compat) | 2 | ✅ | ✅ | ✅ | ✅ | ✅ |
| DataFusion + Local FS (dev) | 2 | ✅ | ✅ | ✅ | ✅ | ✅ |

---

## Client Compatibility

| Client | Protocol | Phase 1 | Phase 2 | Phase 3 |
|--------|----------|---------|---------|---------|
| HTTP REST (curl, httpie) | Databricks REST API | ✅ | ✅ | ✅ |
| Python `databricks-sql-connector` | REST | ✅ | ✅ | ✅ |
| Databricks JDBC Driver (REST mode) | REST | ✅ | ✅ | ✅ |
| Standard JDBC (PostgreSQL driver) | pgwire | — | — | ✅ |
| psql CLI | pgwire | — | — | ✅ |
| dbt (databricks adapter) | REST | ✅ | ✅ | ✅ |
| Power BI (Databricks connector) | REST | ✅ | ✅ | ✅ |
| Tableau (Databricks connector) | REST | ✅ | ✅ | ✅ |

---

## Design Principles

1. **Wire-compatible first** — Client ไม่ต้องเปลี่ยนอะไรเลย ทุก backend ใหม่ transparent
2. **Zero-dependency domain** — Domain layer ไม่ depend บน Databricks, DataFusion, หรือ HTTP ใดๆ
3. **Single binary** — Deploy ได้เป็น binary เดียว หรือ Docker image เดียว ไม่ต้อง orchestrate หลาย service
4. **Config-driven backends** — เพิ่ม/เปลี่ยน backend ผ่าน config ไม่ต้อง recompile
5. **Rust performance** — เป้าหมาย proxy overhead < 10ms P99, DataFusion throughput > 500MB/s

---

*Document Version: 1.0 | Created: 2026-03-06 | สร้างโดย: Claude (Cowork mode)*
