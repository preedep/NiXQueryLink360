# NiXQueryLink360 — Phase 1 Requirements

**Project:** Universal SQL Gateway (alternative SQL Endpoint for Databricks)
**Phase:** 1 — Foundation & Databricks Proxy Engine
**Date:** 2026-03-06
**Status:** 🟡 In Progress (implementation complete, testing in progress)

> **Context:** Phase 1 builds the foundational HTTP server and the **first backend** (Databricks proxy).
> The clean architecture established here — especially the `WarehouseClient` port trait — is designed
> so Phase 2 can add the DataFusion embedded engine as a second backend with zero changes to the
> HTTP or application layers.

---

## 1. Overview & Goals

NiXQueryLink360 เป็น Universal SQL Gateway ที่ compatible กับ Databricks Statement Execution API v2.0.
Phase 1 มีเป้าหมายหลักคือ:

- สร้าง HTTP server ที่ expose REST API ที่ compatible กับ Databricks SQL Statement Execution API v2.0
- รองรับ **Databricks Proxy Mode** — รับ SQL request จาก client แล้ว forward ไปยัง Databricks SQL Warehouse จริง พร้อมจัดการ connection pooling, retry, และ error normalization
- วาง **clean architecture foundation** (domain / application / infrastructure / interfaces) ที่รองรับการเพิ่ม backend ใหม่ใน Phase 2 โดยไม่ต้องแก้ HTTP layer

### Target Users (Phase 1)

- ทีมที่ต้องการ unified auth layer และ audit logging หน้า Databricks
- ทีมที่ต้องการลด latency โดยมี connection pooling layer
- Developer ที่ต้องการ local testing endpoint

---

## 2. Architecture Overview

```
Client (REST / BI tools)
        │
        │  Databricks SQL Statement Execution API v2.0
        ▼
┌─────────────────────────────────────────┐
│          NiXQueryLink360                │
│                                         │
│  ┌─────────────┐   ┌─────────────────┐  │
│  │ HTTP Layer  │──▶│ Application     │  │
│  │ (Axum 0.8)  │   │ Use Cases       │  │
│  │             │   │ submit/get/     │  │
│  │ middleware: │   │ cancel          │  │
│  │ - auth      │   └────────┬────────┘  │
│  │ - trace     │            │           │
│  │ - request-id│   ┌────────▼────────┐  │
│  └─────────────┘   │ WarehouseClient │  │
│                    │ (Port Trait)    │  │
│                    └────────┬────────┘  │
│                             │           │
│                    ┌────────▼────────┐  │
│                    │ DatabricksClient│  │
│                    │ (Phase 1 impl)  │  │
│                    │ reqwest + retry │  │
│                    └────────┬────────┘  │
└────────────────────────────┼────────────┘
                             ▼
                 Databricks SQL Warehouse
                 (Statement Execution API v2.0)
```

**Port trait `WarehouseClient`** คือจุดที่ Phase 2 จะ plug DataFusion engine เข้ามาแทน Databricks client โดยที่ HTTP layer และ use cases ไม่เปลี่ยนแปลงเลย

---

## 3. Functional Requirements

### 3.1 HTTP Server

| ID | Requirement | Status |
|----|-------------|--------|
| F-001 | Server ต้องรันบน configurable port (default: `8360`) | ✅ |
| F-002 | รองรับ HTTP/1.1 | ✅ |
| F-003 | มี graceful shutdown (handle SIGTERM/SIGINT) | ✅ |
| F-004 | รองรับ TLS/HTTPS ผ่าน reverse proxy (Phase 1) | — |
| F-005 | มี request/response logging middleware (structured JSON log) | ✅ |
| F-006 | มี timeout ต่อ request (configurable, default 30s) | ✅ |
| F-007 | มี `X-Request-ID` correlation header (auto-generate UUID) | ✅ |

### 3.2 API Endpoints — Databricks Statement Execution API v2.0 Compatible

#### 3.2.1 Core Endpoints

| Method | Path | Description | Status |
|--------|------|-------------|--------|
| `POST` | `/api/2.0/sql/statements` | Submit SQL statement | ✅ |
| `GET` | `/api/2.0/sql/statements/{statement_id}` | Get statement status & results | ✅ |
| `DELETE` | `/api/2.0/sql/statements/{statement_id}/cancel` | Cancel running statement | ✅ |
| `GET` | `/health` | Liveness probe | ✅ |
| `GET` | `/ready` | Readiness probe | ✅ |

#### 3.2.2 POST /api/2.0/sql/statements

**Request Body:**
```json
{
  "statement": "SELECT * FROM my_table LIMIT 100",
  "warehouse_id": "wh-prod",
  "wait_timeout": "10s",
  "on_wait_timeout": "CONTINUE",
  "format": "JSON_ARRAY",
  "disposition": "INLINE",
  "parameters": [
    { "name": "p1", "value": "foo", "type": "STRING" }
  ]
}
```

| ID | Requirement | Status |
|----|-------------|--------|
| F-101 | รับและ validate request body ตาม Databricks schema | ✅ |
| F-102 | รองรับ `format`: `JSON_ARRAY` (Phase 1), `ARROW_STREAM` (Phase 2) | ✅ |
| F-103 | รองรับ `disposition`: `INLINE` (Phase 1), `EXTERNAL_LINKS` (Phase 2) | ✅ |
| F-104 | รองรับ `wait_timeout` (0s–50s) และ `on_wait_timeout` (`CONTINUE`/`CANCEL`) | ✅ |
| F-105 | Forward request ไปยัง Databricks Warehouse ตาม `warehouse_id` | ✅ |
| F-106 | Return `statement_id` กลับทันที (async mode) | ✅ |
| F-107 | รองรับ parameterized query (named parameters) | ✅ |

#### 3.2.3 GET /api/2.0/sql/statements/{statement_id}

| ID | Requirement | Status |
|----|-------------|--------|
| F-201 | Return statement status: `PENDING`, `RUNNING`, `SUCCEEDED`, `FAILED`, `CANCELLED`, `CLOSED` | ✅ |
| F-202 | ถ้า SUCCEEDED: return result set inline (JSON_ARRAY format) | ✅ |
| F-203 | ถ้า FAILED: return error message และ error code ที่ compatible กับ Databricks | ✅ |
| F-204 | ถ้า statement_id ไม่มี: return 404 | ✅ |

#### 3.2.4 DELETE /api/2.0/sql/statements/{statement_id}/cancel

| ID | Requirement | Status |
|----|-------------|--------|
| F-301 | ส่ง cancel request ไปยัง Databricks และ return 200 OK ถ้าสำเร็จ | ✅ |
| F-302 | ถ้า upstream error: propagate error ที่ compatible กับ Databricks spec | ✅ |

### 3.3 Authentication & Authorization

| ID | Requirement | Status |
|----|-------------|--------|
| F-401 | รองรับ **Personal Access Token (PAT)** ใน `Authorization: Bearer <token>` header | ✅ |
| F-402 | Server-side token injection — ดึง token จาก env var ตาม warehouse (override caller token) | ✅ |
| F-403 | Reject request ที่ไม่มี valid auth ด้วย HTTP 401 | ✅ |
| F-404 | Log auth failures (ไม่ log token จริง) | ✅ |
| F-405 | รองรับ multiple upstream targets แต่ละตัวมี token แยกกัน | ✅ |

### 3.4 Connection Pooling & Upstream Management

| ID | Requirement | Status |
|----|-------------|--------|
| F-501 | HTTP connection pool ไปยัง Databricks (`reqwest` + connection pooling) | ✅ |
| F-502 | รองรับ multiple warehouse targets (routing ตาม `warehouse_id`) | ✅ |
| F-503 | Retry logic สำหรับ transient errors (5xx, 429, timeout) — max 3 retries, exponential backoff | ✅ |
| F-504 | Configurable upstream timeout (default 120s สำหรับ long-running queries) | ✅ |
| F-505 | HTTPS-only upstream connections | ✅ |

### 3.5 Configuration Management

| ID | Requirement | Status |
|----|-------------|--------|
| F-601 | อ่าน config จาก **TOML file** (`config.toml`) | ✅ |
| F-602 | Override ผ่าน **environment variables** (prefix: `NQL__`, double-underscore separator) | ✅ |
| F-603 | Validate config ตอน startup — fail fast ถ้า config ไม่ถูกต้อง | ✅ |
| F-604 | ไม่ log sensitive values (token, password) | ✅ |

**config.toml:**
```toml
[server]
host = "0.0.0.0"
port = 8360
request_timeout_secs = 30

[upstream]
default_warehouse_id = "wh-prod"

[[upstream.warehouses]]
id        = "wh-prod"
host      = "adb-xxxx.azuredatabricks.net"
http_path = "/sql/1.0/warehouses/abc123"
token_env = "DATABRICKS_TOKEN_PROD"

[pool]
max_connections       = 50
connection_timeout_secs = 10
idle_timeout_secs     = 300

[retry]
max_attempts  = 3
base_delay_ms = 500

[logging]
level  = "info"   # trace | debug | info | warn | error
format = "json"   # json | pretty
```

### 3.6 Observability

| ID | Requirement | Status |
|----|-------------|--------|
| F-701 | Structured logging ด้วย `tracing` crate (JSON format ใน production) | ✅ |
| F-702 | Log ทุก request: method, path, status code, elapsed_ms | ✅ |
| F-703 | Log correlation ID (`X-Request-ID` header) | ✅ |
| F-704 | Log upstream latency แยกจาก total latency | ✅ |
| F-705 | `/health` และ `/ready` endpoints | ✅ |

---

## 4. Non-Functional Requirements

### 4.1 Performance

| ID | Requirement | Target |
|----|-------------|--------|
| NF-101 | Proxy overhead (latency added บน Databricks) | < 10ms P99 |
| NF-102 | Concurrent connections | >= 200 |
| NF-103 | Memory footprint ขณะ idle | < 50MB |
| NF-104 | Cold start time | < 2 วินาที |

### 4.2 Reliability

| ID | Requirement |
|----|-------------|
| NF-201 | Graceful shutdown — drain in-flight requests ก่อน terminate |
| NF-202 | ไม่ crash เมื่อ upstream ไม่ตอบ (timeout gracefully) |
| NF-203 | ไม่ leak memory เมื่อรันนานกว่า 24 ชั่วโมง |

### 4.3 Security

| ID | Requirement |
|----|-------------|
| NF-301 | ไม่เก็บ token ใน log, error message, หรือ response body |
| NF-302 | Reject request body ที่ใหญ่กว่า 10MB ด้วย 413 |
| NF-303 | HTTPS-only upstream connections (reqwest `.https_only(true)`) |

### 4.4 Developer Experience

| ID | Requirement |
|----|-------------|
| NF-401 | `cargo build --release` สำเร็จโดยไม่มี warning |
| NF-402 | Unit test coverage >= 60% สำหรับ business logic |
| NF-403 | Integration test ที่ใช้ mock upstream (ไม่ต้องเชื่อม Databricks จริง) |
| NF-404 | Docker image ขนาดไม่เกิน 50MB (multi-stage build) |

---

## 5. Technical Stack

| Component | Crate | Version |
|-----------|-------|---------|
| Async Runtime | `tokio` | 1 |
| HTTP Server | `axum` | 0.8 |
| HTTP Middleware | `tower-http` | 0.6 |
| HTTP Client | `reqwest` | 0.12 |
| Serialization | `serde` + `serde_json` | 1 |
| Config | `config` | 0.15 |
| Logging | `tracing` + `tracing-subscriber` | 0.1 / 0.3 |
| Error Handling | `thiserror` + `anyhow` | 2 / 1 |
| UUID | `uuid` | 1 |
| Async Trait | `async-trait` | 0.1 |
| Test Mock | `mockito` | 1 |

---

## 6. Project Structure

```
NiXQueryLink360/
├── Cargo.toml
├── config.toml.example
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── domain/
│   │   ├── entities/        # Statement, WarehouseConfig
│   │   ├── errors.rs        # DomainError enum
│   │   └── ports/           # WarehouseClient trait ← Phase 2 plugs in here
│   ├── application/
│   │   └── use_cases/       # submit, get, cancel
│   ├── infrastructure/
│   │   ├── config/          # Settings, loader
│   │   └── http_client/     # DatabricksClient, RetryPolicy
│   └── interfaces/
│       ├── dto/             # request/response DTOs
│       └── http/            # handlers, middleware, router
```

---

## 7. Out of Scope (Phase 1 → Moved to Phase 2+)

| Item | Target Phase |
|------|-------------|
| DataFusion embedded query engine | Phase 2 |
| S3 / Azure Blob / GCS object storage backend | Phase 2 |
| Delta Lake / Apache Iceberg table format | Phase 2 |
| Query Result Caching | Phase 4 |
| PostgreSQL wire protocol (JDBC/ODBC) | Phase 3 |
| OAuth 2.0 / OIDC auth flows | Phase 4 |
| ARROW_STREAM result format | Phase 2 |
| Prometheus metrics endpoint | Phase 2 |
| Query Rewriting / SQL transformation | Phase 3+ |
| Web UI / Dashboard | Phase 4 |

---

## 8. Acceptance Criteria (Phase 1 Complete)

- [ ] `cargo build --release` สำเร็จโดยไม่มี warning
- [ ] Server รันและตอบสนอง `/health` ด้วย HTTP 200
- [ ] Submit SQL ผ่าน `POST /api/2.0/sql/statements` และรับ result กลับได้
- [ ] Request โดยไม่มี `Authorization` header → HTTP 401
- [ ] Structured log ออกมาในรูป JSON พร้อม request ID และ elapsed_ms
- [ ] Unit tests ผ่านทั้งหมด (`cargo test`)
- [ ] Integration test กับ mock upstream ผ่านทั้งหมด
- [ ] Docker image build ได้และ run ได้

---

*Document Version: 1.1 | Updated: 2026-03-06 | สร้างโดย: Claude (Cowork mode)*
