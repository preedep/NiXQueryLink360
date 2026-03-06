# NiXQueryLink360 — Phase 1 Requirements

**Project:** Alternative SQL Endpoint for Databricks (Rust)
**Phase:** 1 — Foundation & Core Proxy Engine
**Date:** 2026-03-06
**Status:** Draft

---

## 1. Overview & Goals

NiXQueryLink360 เป็น alternative SQL Endpoint ที่ compatible กับ Databricks Statement Execution API โดย Phase 1 มีเป้าหมายหลักคือ:

- สร้าง HTTP server ที่ expose REST API ที่ compatible กับ Databricks SQL Statement Execution API v2.0
- รองรับ **Proxy Mode** — รับ SQL request จาก client แล้ว forward ไปยัง Databricks SQL Warehouse จริง พร้อมจัดการ connection pooling, retry, และ error normalization
- วาง foundation สำหรับ Phase 2 ที่จะเพิ่ม Direct Delta Lake Mode (bypass Databricks Warehouse โดยสมบูรณ์)

### Target Users (Phase 1)

- ทีมที่ต้องการลด latency โดยมี caching/pooling layer หน้า Databricks
- ทีมที่ต้องการ unified auth layer และ audit logging
- Developer ที่ต้องการ local testing endpoint โดยไม่ต้องพึ่ง Databricks ตลอดเวลา

---

## 2. Architecture Overview

```
Client (JDBC/ODBC/REST)
        │
        ▼
┌─────────────────────────────────────┐
│         NiXQueryLink360             │
│                                     │
│  ┌──────────┐    ┌───────────────┐  │
│  │ HTTP API │───▶│  Query Engine │  │
│  │  (Axum)  │    │  (Phase 1:    │  │
│  └──────────┘    │  Proxy Mode)  │  │
│                  └──────┬────────┘  │
│  ┌──────────┐           │           │
│  │  Auth    │    ┌──────▼────────┐  │
│  │  Layer   │    │ Conn Pool /   │  │
│  └──────────┘    │ HTTP Client   │  │
│                  └──────┬────────┘  │
│  ┌──────────┐           │           │
│  │  Config  │           │           │
│  │  Manager │           │           │
│  └──────────┘           │           │
└─────────────────────────┼───────────┘
                          ▼
              Databricks SQL Warehouse
              (Statement Execution API v2.0)
```

---

## 3. Functional Requirements

### 3.1 HTTP Server

| ID | Requirement |
|----|-------------|
| F-001 | Server ต้องรันบน configurable port (default: `8360`) |
| F-002 | รองรับ HTTP/1.1 และ HTTP/2 |
| F-003 | มี graceful shutdown (handle SIGTERM/SIGINT) |
| F-004 | รองรับ TLS/HTTPS ผ่าน config (optional ใน Phase 1, ใช้ reverse proxy ได้) |
| F-005 | มี request/response logging middleware (structured JSON log) |
| F-006 | มี timeout ต่อ request (configurable, default 30s) |

### 3.2 API Endpoints — Databricks Statement Execution API v2.0 Compatible

#### 3.2.1 Core Endpoints (Must Have)

| Method | Path | คำอธิบาย |
|--------|------|----------|
| `POST` | `/api/2.0/sql/statements` | Submit SQL statement |
| `GET` | `/api/2.0/sql/statements/{statement_id}` | Get statement status & results |
| `DELETE` | `/api/2.0/sql/statements/{statement_id}/cancel` | Cancel running statement |
| `GET` | `/health` | Health check (non-Databricks, สำหรับ load balancer) |
| `GET` | `/metrics` | Prometheus-compatible metrics (basic) |

#### 3.2.2 POST /api/2.0/sql/statements — Submit Statement

**Request Body:**
```json
{
  "statement": "SELECT * FROM my_table LIMIT 100",
  "warehouse_id": "abc123",
  "wait_timeout": "10s",
  "on_wait_timeout": "CONTINUE",
  "format": "JSON_ARRAY",
  "disposition": "INLINE",
  "parameters": [
    { "name": "p1", "value": "foo", "type": "STRING" }
  ]
}
```

**Requirements:**
| ID | Requirement |
|----|-------------|
| F-101 | รับและ validate request body ตาม Databricks schema |
| F-102 | รองรับ `format`: `JSON_ARRAY`, `ARROW_STREAM` (Phase 1: JSON_ARRAY เป็น priority) |
| F-103 | รองรับ `disposition`: `INLINE` และ `EXTERNAL_LINKS` (Phase 1: INLINE) |
| F-104 | รองรับ `wait_timeout` (0s – 50s) และ `on_wait_timeout` (`CONTINUE` / `CANCEL`) |
| F-105 | Forward request ไปยัง upstream Databricks Warehouse ตาม routing config |
| F-106 | Return statement_id ที่ unique และ traceable กลับมาทันที (ถ้า async) |
| F-107 | รองรับ parameterized query (named parameters) |

#### 3.2.3 GET /api/2.0/sql/statements/{statement_id}

**Requirements:**
| ID | Requirement |
|----|-------------|
| F-201 | Return statement status: `PENDING`, `RUNNING`, `SUCCEEDED`, `FAILED`, `CANCELLED`, `CLOSED` |
| F-202 | ถ้า SUCCEEDED: return result set inline (JSON_ARRAY format) |
| F-203 | รองรับ pagination ผ่าน `chunk_index` |
| F-204 | ถ้า FAILED: return error message และ error code ที่ compatible กับ Databricks |

#### 3.2.4 DELETE /api/2.0/sql/statements/{statement_id}/cancel

| ID | Requirement |
|----|-------------|
| F-301 | ส่ง cancel request ไปยัง upstream และ return 200 OK ถ้าสำเร็จ |
| F-302 | ถ้า statement ไม่มีอยู่หรือ already completed — return error 404/400 ตาม Databricks spec |

### 3.3 Authentication & Authorization

| ID | Requirement |
|----|-------------|
| F-401 | รองรับ **Personal Access Token (PAT)** ใน `Authorization: Bearer <token>` header |
| F-402 | รองรับ config ให้ใช้ token จาก environment variable แทน client token (server-side token injection) |
| F-403 | Reject request ที่ไม่มี valid auth ด้วย HTTP 401 |
| F-404 | Log auth failures พร้อม IP และ timestamp (ไม่ log token จริง) |
| F-405 | รองรับ multiple upstream targets แต่ละตัวมี token แยกกัน |

### 3.4 Connection Pooling & Upstream Management

| ID | Requirement |
|----|-------------|
| F-501 | มี HTTP connection pool ไปยัง Databricks (ใช้ `reqwest` + connection pooling) |
| F-502 | รองรับ multiple warehouse targets (routing ตาม `warehouse_id` ใน request) |
| F-503 | มี retry logic สำหรับ transient errors (5xx, timeout) — max 3 retries, exponential backoff |
| F-504 | มี circuit breaker อย่างง่าย (ถ้า upstream ล้มเหลวเกิน threshold — reject request ทันที) |
| F-505 | รองรับ configurable upstream timeout (default 120s สำหรับ long-running queries) |

### 3.5 Configuration Management

| ID | Requirement |
|----|-------------|
| F-601 | อ่าน config จาก **TOML file** (`config.toml`) |
| F-602 | รองรับ override ผ่าน **environment variables** (prefix: `NQL_`) |
| F-603 | Validate config ตอน startup — fail fast ถ้า config ไม่ถูกต้อง |
| F-604 | ต้องไม่ log sensitive values (token, password) |

**ตัวอย่าง config.toml:**
```toml
[server]
host = "0.0.0.0"
port = 8360
request_timeout_secs = 30

[upstream]
default_warehouse_id = "abc123"

[[upstream.warehouses]]
id = "abc123"
host = "adb-xxxx.azuredatabricks.net"
http_path = "/sql/1.0/warehouses/abc123"
token_env = "DATABRICKS_TOKEN_PROD"

[[upstream.warehouses]]
id = "dev456"
host = "adb-yyyy.azuredatabricks.net"
http_path = "/sql/1.0/warehouses/dev456"
token_env = "DATABRICKS_TOKEN_DEV"

[pool]
max_connections = 50
connection_timeout_secs = 10
idle_timeout_secs = 300

[retry]
max_attempts = 3
base_delay_ms = 500

[logging]
level = "info"   # trace | debug | info | warn | error
format = "json"  # json | pretty
```

### 3.6 Observability

| ID | Requirement |
|----|-------------|
| F-701 | Structured logging ด้วย `tracing` crate (JSON format ใน production) |
| F-702 | Log ทุก request: method, path, status code, latency, upstream latency |
| F-703 | Log correlation ID (จาก `X-Request-ID` header หรือ auto-generate) |
| F-704 | Expose `/metrics` endpoint ที่มี basic Prometheus metrics: request count, latency histogram, upstream error rate |
| F-705 | มี `/health` endpoint ที่ return upstream connectivity status |

---

## 4. Non-Functional Requirements

### 4.1 Performance

| ID | Requirement | Target |
|----|-------------|--------|
| NF-101 | Request overhead (proxy latency added บนของ Databricks) | < 10ms P99 |
| NF-102 | Concurrent connections รองรับพร้อมกัน | >= 200 |
| NF-103 | Memory footprint ขณะ idle | < 50MB |
| NF-104 | Cold start time | < 2 วินาที |

### 4.2 Reliability

| ID | Requirement |
|----|-------------|
| NF-201 | Graceful shutdown — drain in-flight requests ก่อน terminate |
| NF-202 | ไม่ crash เมื่อ upstream Databricks ไม่ตอบ (timeout gracefully) |
| NF-203 | ไม่ leak memory เมื่อรันต่อเนื่องนานกว่า 24 ชั่วโมง |

### 4.3 Security

| ID | Requirement |
|----|-------------|
| NF-301 | ไม่เก็บ token ใน log, error message, หรือ response body |
| NF-302 | Request body ที่ใหญ่กว่า 10MB ให้ reject ด้วย 413 |
| NF-303 | มี rate limiting อย่างง่ายต่อ IP (configurable, default: 100 req/min) |

### 4.4 Developer Experience

| ID | Requirement |
|----|-------------|
| NF-401 | Build ด้วย `cargo build --release` โดยไม่มี error หรือ warning |
| NF-402 | มี unit tests coverage >= 60% สำหรับ business logic |
| NF-403 | มี integration test ที่ใช้ mock upstream (ไม่ต้องเชื่อม Databricks จริงเพื่อ test) |
| NF-404 | Docker image ขนาดไม่เกิน 50MB (ใช้ multi-stage build) |
| NF-405 | มี `Makefile` หรือ `justfile` สำหรับ common commands |

---

## 5. Technical Stack (Recommended)

| Component | Crate | เหตุผล |
|-----------|-------|--------|
| Async Runtime | `tokio` | มาตรฐาน Rust async |
| HTTP Server | `axum` | Ergonomic, tower ecosystem |
| HTTP Client | `reqwest` | Full-featured, async |
| Serialization | `serde` + `serde_json` | Standard |
| Config | `config` crate | รองรับ TOML + env override |
| Logging | `tracing` + `tracing-subscriber` | Structured, async-friendly |
| Metrics | `prometheus` | Standard format |
| Error Handling | `thiserror` + `anyhow` | Ergonomic error types |
| Arrow Format | `arrow2` หรือ `arrow` | สำหรับ ARROW_STREAM (Phase 2) |
| Testing | `tokio-test`, `mockito` | Mock HTTP server |

---

## 6. Project Structure (Recommended)

```
NiXQueryLink360/
├── Cargo.toml
├── Cargo.lock
├── config.toml.example
├── Dockerfile
├── Makefile
├── README.md
│
└── src/
    ├── main.rs               # Entry point, startup
    ├── config.rs             # Config loading & validation
    ├── error.rs              # Custom error types
    │
    ├── server/
    │   ├── mod.rs            # Axum router setup
    │   ├── middleware.rs     # Auth, logging, rate limit
    │   └── handlers/
    │       ├── mod.rs
    │       ├── statements.rs # POST/GET/DELETE /sql/statements
    │       └── health.rs     # /health, /metrics
    │
    ├── proxy/
    │   ├── mod.rs            # Proxy engine core
    │   ├── client.rs         # reqwest client + pool
    │   ├── routing.rs        # Warehouse routing logic
    │   └── retry.rs          # Retry + circuit breaker
    │
    └── models/
        ├── mod.rs
        ├── request.rs        # StatementRequest, etc.
        └── response.rs       # StatementResponse, ResultData, etc.
```

---

## 7. Out of Scope (Phase 1)

สิ่งต่อไปนี้จะทำใน Phase 2 หรือหลังจากนั้น:

- **Direct Delta Lake Mode** — อ่าน Delta table โดยตรง (ไม่ผ่าน Databricks Warehouse)
- **Query Result Caching** — cache ผล query ซ้ำใน Redis/in-memory
- **JDBC Driver** — สร้าง JDBC driver ที่ point มายัง NiXQueryLink360
- **OAuth 2.0 / OIDC** — richer auth flows
- **ARROW_STREAM format** — binary Arrow result format (Phase 1 ให้ JSON_ARRAY ก่อน)
- **Query Rewriting** — parse และ transform SQL ก่อน forward
- **Dashboard / UI** — web interface สำหรับ monitoring

---

## 8. Acceptance Criteria (Phase 1 Complete)

Phase 1 ถือว่าสมบูรณ์เมื่อ:

- [ ] `cargo build --release` สำเร็จโดยไม่มี warning
- [ ] Server รันและตอบสนอง `/health` ด้วย HTTP 200
- [ ] สามารถ submit SQL query ผ่าน `POST /api/2.0/sql/statements` และรับ result กลับได้
- [ ] ส่ง request โดยไม่มี `Authorization` header → ได้ HTTP 401
- [ ] Structured log ออกมาในรูป JSON พร้อม request ID
- [ ] Unit test ผ่านทั้งหมด (`cargo test`)
- [ ] Integration test กับ mock upstream ผ่านทั้งหมด
- [ ] Docker image build ได้และ run ได้

---

## 9. Milestones Suggestion

| Milestone | งาน | ระยะเวลาประมาณ |
|-----------|-----|----------------|
| M1 | Project structure, Cargo.toml dependencies, config loader | 2–3 วัน |
| M2 | Axum HTTP server + health/metrics endpoints + middleware skeleton | 3–4 วัน |
| M3 | Request/response models (serde) + auth middleware | 2–3 วัน |
| M4 | reqwest client + upstream proxy logic + routing | 4–5 วัน |
| M5 | Retry logic + basic circuit breaker | 2–3 วัน |
| M6 | Structured logging + Prometheus metrics | 2–3 วัน |
| M7 | Unit tests + integration tests (mock upstream) | 3–4 วัน |
| M8 | Dockerfile + Makefile + documentation | 2 วัน |
| **Total** | | **~3–4 สัปดาห์** |

---

*Document Version: 1.0 | สร้างโดย: Claude (Cowork mode)*
