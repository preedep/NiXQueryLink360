# NiXQueryLink360 — Phase 2 Requirements

**Project:** Universal SQL Gateway
**Phase:** 2 — DataFusion Embedded Engine + Object Storage Backend
**Date:** 2026-03-06
**Status:** 📋 Planned (prerequisite: Phase 1 complete)

---

## 1. Overview & Goals

Phase 2 เพิ่ม **backend ที่สอง** เข้าสู่ NiXQueryLink360 — แทนที่จะ forward query ไป Databricks เสมอ ระบบสามารถ **execute SQL ด้วยตัวเอง** โดยใช้ Apache DataFusion และ query โดยตรงจาก object storage (S3, Azure Blob, GCS) ใน format มาตรฐาน (Parquet, Delta Lake, Iceberg)

เป้าหมายหลัก:

1. **DataFusion Engine** — embed Apache DataFusion เป็น SQL query engine ภายใน binary
2. **Object Storage Backend** — รองรับ S3, Azure Blob Storage, GCS ผ่าน `object_store` crate
3. **Table Formats** — รองรับ Parquet, Delta Lake, Apache Iceberg
4. **Multi-Backend Routing** — route request ตาม `warehouse_id` ไปยัง Databricks หรือ DataFusion ได้ใน instance เดียวกัน
5. **Wire Compatibility** — client ใช้ Databricks API เหมือนเดิม ไม่ต้องเปลี่ยนอะไร

---

## 2. Architecture Overview

```
Client (REST / BI tools / JDBC)
              │
              │  Databricks SQL Statement Execution API v2.0
              ▼
┌─────────────────────────────────────────────────────┐
│                NiXQueryLink360                      │
│                                                     │
│  ┌───────────────┐   ┌───────────────────────────┐  │
│  │  HTTP Layer   │──▶│    Application Use Cases  │  │
│  │  (Axum 0.8)   │   │  submit / get / cancel    │  │
│  └───────────────┘   └─────────────┬─────────────┘  │
│                                    │                 │
│                        ┌───────────▼───────────┐     │
│                        │   Query Router        │     │
│                        │   (warehouse_id       │     │
│                        │    → backend type)    │     │
│                        └──────┬────────┬───────┘     │
│                               │        │             │
│                ┌──────────────▼─┐  ┌───▼───────────┐ │
│                │ DatabricksClient│  │DataFusionClient│ │
│                │ (Phase 1 ✅)  │  │ (Phase 2)     │ │
│                │ reqwest proxy  │  │ embedded SQL  │ │
│                └───────┬────────┘  └───────┬───────┘ │
└────────────────────────┼───────────────────┼─────────┘
                         ▼                   ▼
               Databricks SQL         Object Storage
               Warehouse              ├── AWS S3
                                      ├── Azure Blob
                                      ├── GCS
                                      │
                                  Table Formats
                                      ├── Apache Parquet
                                      ├── Delta Lake (delta-rs)
                                      ├── Apache Iceberg
                                      └── CSV / JSON / ORC
```

> **ไม่มีการเปลี่ยน HTTP layer หรือ application layer** — Phase 2 เพิ่มเฉพาะ infrastructure implementation ใหม่ที่ implement `WarehouseClient` trait เดิม

---

## 3. Functional Requirements

### 3.1 Multi-Backend Query Router

| ID | Requirement |
|----|-------------|
| F2-001 | Config `warehouse_id` แต่ละตัวต้องระบุ `backend_type`: `"databricks"` หรือ `"datafusion"` |
| F2-002 | Router เลือก backend ตาม `warehouse_id` ในแต่ละ request |
| F2-003 | ถ้า `warehouse_id` ไม่มีใน config → return `DomainError::WarehouseNotFound` (พฤติกรรมเดิม) |
| F2-004 | สามารถมีทั้ง Databricks และ DataFusion warehouses ใน instance เดียวกัน |
| F2-005 | `default_warehouse_id` ชี้ไปยัง backend ไหนก็ได้ |

**config.toml (Phase 2):**
```toml
[upstream]
default_warehouse_id = "wh-s3-prod"

# Databricks backend (Phase 1)
[[upstream.warehouses]]
id           = "wh-databricks-prod"
backend_type = "databricks"
host         = "adb-xxxx.azuredatabricks.net"
http_path    = "/sql/1.0/warehouses/abc123"
token_env    = "DATABRICKS_TOKEN_PROD"

# DataFusion backend — S3 + Delta Lake (Phase 2)
[[upstream.warehouses]]
id           = "wh-s3-prod"
backend_type = "datafusion"
catalog      = "prod_catalog"

# DataFusion backend — Azure Blob + Parquet
[[upstream.warehouses]]
id           = "wh-azure-analytics"
backend_type = "datafusion"
catalog      = "analytics_catalog"
```

---

### 3.2 DataFusion Engine

| ID | Requirement |
|----|-------------|
| F2-101 | Embed Apache DataFusion เป็น in-process query engine (ไม่ใช่ external service) |
| F2-102 | รองรับ ANSI SQL ที่ DataFusion รองรับ (SELECT, JOIN, GROUP BY, WINDOW, CTE, subquery) |
| F2-103 | Execute query แบบ async บน Tokio runtime |
| F2-104 | แปลง DataFusion `RecordBatch` (Arrow) → `StatementResult` (JSON_ARRAY) |
| F2-105 | แปลง DataFusion `RecordBatch` → ARROW_STREAM format (binary) |
| F2-106 | Map DataFusion errors → `DomainError` variants ที่มีอยู่ |
| F2-107 | สร้าง `DataFusionClient` struct ที่ implement `WarehouseClient` trait |
| F2-108 | Statement ID สำหรับ DataFusion queries ต้องเป็น UUID ที่ track ได้ใน in-memory store |
| F2-109 | รองรับ async execution: submit → polling จนกว่า query จะ complete |
| F2-110 | Query timeout ต่อ request (configurable, default 300s) |

### 3.3 Object Storage Support

| ID | Requirement |
|----|-------------|
| F2-201 | รองรับ **AWS S3** ผ่าน `object_store` crate |
| F2-202 | รองรับ **Azure Blob Storage** ผ่าน `object_store` crate |
| F2-203 | รองรับ **Google Cloud Storage** ผ่าน `object_store` crate (nice to have) |
| F2-204 | รองรับ **Local filesystem** สำหรับ development/testing (`file://` path) |
| F2-205 | Authentication ผ่าน standard mechanisms: IAM Role (S3), Managed Identity (Azure), Service Account (GCS) |
| F2-206 | รองรับ explicit credentials ผ่าน environment variables (fallback จาก IAM) |
| F2-207 | รองรับ custom endpoint URL สำหรับ S3-compatible storage (MinIO, Ceph, etc.) |

**Auth environment variables:**
```bash
# AWS S3
AWS_ACCESS_KEY_ID=...
AWS_SECRET_ACCESS_KEY=...
AWS_DEFAULT_REGION=ap-southeast-1
# (หรือใช้ IAM Role — ไม่ต้องตั้งค่า)

# Azure Blob
AZURE_STORAGE_ACCOUNT_NAME=...
AZURE_STORAGE_ACCOUNT_KEY=...
# (หรือ AZURE_CLIENT_ID + AZURE_CLIENT_SECRET + AZURE_TENANT_ID สำหรับ Managed Identity)

# S3-compatible (MinIO)
AWS_ENDPOINT_URL=http://minio:9000
AWS_ACCESS_KEY_ID=minioadmin
AWS_SECRET_ACCESS_KEY=minioadmin
```

### 3.4 Table Format Support

#### 3.4.1 Apache Parquet (Must Have)

| ID | Requirement |
|----|-------------|
| F2-301 | อ่าน Parquet files จาก object storage โดยตรง |
| F2-302 | รองรับ Parquet schema evolution (เพิ่ม/ลด column) |
| F2-303 | รองรับ Parquet partitioning (Hive-style partition directories) |
| F2-304 | Predicate pushdown — filter ลง Parquet row group ก่อน load |
| F2-305 | Column pruning — อ่านเฉพาะ column ที่ SELECT ต้องการ |
| F2-306 | Register Parquet path เป็น table ใน config |

**config.toml — Parquet table:**
```toml
[[catalogs.prod_catalog.tables]]
name   = "orders"
format = "parquet"
location = "s3://my-bucket/data/orders/"
partition_columns = ["year", "month"]
```

#### 3.4.2 Delta Lake (Must Have)

| ID | Requirement |
|----|-------------|
| F2-401 | อ่าน Delta Lake table ผ่าน `deltalake` (delta-rs) crate |
| F2-402 | รองรับ time travel: `SELECT * FROM orders VERSION AS OF 5` |
| F2-403 | รองรับ time travel by timestamp: `TIMESTAMP AS OF '2025-01-01'` |
| F2-404 | อ่าน Delta transaction log เพื่อ resolve latest snapshot |
| F2-405 | รองรับ Delta table statistics สำหรับ query optimization |
| F2-406 | Register Delta table location เป็น table ใน config |
| F2-407 | Checkpoint-aware reading (อ่าน checkpoint แทน log ยาว) |

**config.toml — Delta Lake table:**
```toml
[[catalogs.prod_catalog.tables]]
name     = "sales_events"
format   = "delta"
location = "s3://my-bucket/delta/sales_events/"
```

#### 3.4.3 Apache Iceberg (Should Have)

| ID | Requirement |
|----|-------------|
| F2-501 | อ่าน Iceberg table ผ่าน `iceberg-rust` crate |
| F2-502 | รองรับ Iceberg catalog: REST catalog, file-based catalog |
| F2-503 | รองรับ Iceberg snapshot isolation |
| F2-504 | Register Iceberg table ใน config |

#### 3.4.4 CSV & JSON (Nice to Have)

| ID | Requirement |
|----|-------------|
| F2-601 | อ่าน gzipped CSV จาก object storage |
| F2-602 | อ่าน NDJSON (newline-delimited JSON) จาก object storage |
| F2-603 | Auto-detect schema จาก CSV/JSON header/sample |

### 3.5 Catalog & Schema Management

| ID | Requirement |
|----|-------------|
| F2-701 | Define catalogs และ tables ใน `config.toml` (static catalog) |
| F2-702 | Register tables เข้า DataFusion `SessionContext` ตอน startup |
| F2-703 | รองรับ cross-catalog query: `SELECT * FROM prod_catalog.orders JOIN dev_catalog.products …` |
| F2-704 | Hot-reload catalog config โดยไม่ต้อง restart server (Phase 2 nice-to-have) |
| F2-705 | Return schema information เมื่อ query `SHOW TABLES` หรือ `DESCRIBE TABLE` |
| F2-706 | Validate table locations ตอน startup — log warning ถ้า location ไม่ accessible |

**config.toml — Full catalog example:**
```toml
[catalogs.prod_catalog]
description = "Production S3 data"

[[catalogs.prod_catalog.tables]]
name     = "orders"
format   = "delta"
location = "s3://prod-bucket/delta/orders/"

[[catalogs.prod_catalog.tables]]
name     = "products"
format   = "parquet"
location = "s3://prod-bucket/parquet/products/"
partition_columns = ["category"]

[catalogs.local_dev]
description = "Local development data"

[[catalogs.local_dev.tables]]
name     = "test_orders"
format   = "parquet"
location = "file:///tmp/test-data/orders/"
```

### 3.6 Statement Lifecycle (DataFusion backend)

เนื่องจาก DataFusion execute แบบ in-process ไม่มี external statement ID เหมือน Databricks
Phase 2 ต้องมี **in-memory statement store** สำหรับ polling:

| ID | Requirement |
|----|-------------|
| F2-801 | Generate UUID เป็น `statement_id` สำหรับทุก DataFusion query |
| F2-802 | เก็บ statement state ใน in-memory store (`HashMap<Uuid, StatementExecution>`) |
| F2-803 | Execute query ใน background Tokio task — handler return statement_id ทันที |
| F2-804 | Client poll `GET /api/2.0/sql/statements/{id}` จนกว่า state จะเป็น SUCCEEDED/FAILED |
| F2-805 | เก็บผลลัพธ์ใน memory (INLINE) หรือ object storage (EXTERNAL_LINKS สำหรับ large result) |
| F2-806 | Evict completed statements จาก memory หลัง TTL ที่ config ได้ (default 1 ชั่วโมง) |
| F2-807 | Cancel DataFusion query ผ่าน Tokio task cancellation token |
| F2-808 | ถ้า server restart — in-flight statements ที่หายไปจะ return 404 (stateless restart OK) |

### 3.7 Result Formats (DataFusion)

| ID | Requirement |
|----|-------------|
| F2-901 | แปลง Arrow `RecordBatch` → `JSON_ARRAY` (array of arrays) ให้ compatible กับ Databricks format |
| F2-902 | แปลง Arrow `RecordBatch` → Apache Arrow IPC stream format (`ARROW_STREAM`) |
| F2-903 | รองรับ pagination สำหรับ large result sets |
| F2-904 | Map Arrow data types → Databricks SQL type strings สำหรับ schema metadata |

**Arrow type → Databricks type mapping:**
| Arrow Type | Databricks SQL Type |
|-----------|---------------------|
| `Int8/16/32/64` | `TINYINT/SMALLINT/INT/BIGINT` |
| `Float32/64` | `FLOAT/DOUBLE` |
| `Utf8/LargeUtf8` | `STRING` |
| `Boolean` | `BOOLEAN` |
| `Date32` | `DATE` |
| `Timestamp` | `TIMESTAMP` |
| `Decimal128` | `DECIMAL(p,s)` |

---

## 4. Non-Functional Requirements

### 4.1 Performance (DataFusion backend)

| ID | Requirement | Target |
|----|-------------|--------|
| NF2-101 | Query overhead vs direct DataFusion call | < 5ms |
| NF2-102 | S3 Parquet scan throughput (อ่าน 1GB Parquet file) | > 500 MB/s |
| NF2-103 | Concurrent DataFusion queries | >= 20 concurrent |
| NF2-104 | Memory per concurrent DataFusion query | < 512MB (configurable) |

### 4.2 Reliability

| ID | Requirement |
|----|-------------|
| NF2-201 | DataFusion query error ต้องไม่ crash server process |
| NF2-202 | OOM ใน single query ต้องไม่กระทบ query อื่น |
| NF2-203 | Network error ระหว่าง read S3 ต้อง return `DomainError::UpstreamError` ที่ meaningful |
| NF2-204 | Graceful shutdown ต้อง cancel in-flight DataFusion tasks |

### 4.3 Security

| ID | Requirement |
|----|-------------|
| NF2-301 | ไม่ log AWS/Azure credentials ในทุกกรณี |
| NF2-302 | S3 bucket/path ที่ query ต้อง validate ว่าอยู่ใน allowed locations (config) |
| NF2-303 | ไม่รองรับ `file://` path ใน production mode (เปิดได้เฉพาะ development flag) |
| NF2-304 | Query result ที่เก็บใน memory ต้องลบเมื่อ client ดึงไปแล้วหรือ TTL หมด |

---

## 5. Technical Stack (Phase 2 additions)

| Component | Crate | Version | Notes |
|-----------|-------|---------|-------|
| Query Engine | `datafusion` | 44+ | Apache DataFusion |
| Object Storage | `object_store` | 0.11+ | S3, Azure, GCS, Local |
| Delta Lake | `deltalake` | 0.22+ | delta-rs (Rust native) |
| Apache Iceberg | `iceberg` | 0.4+ | iceberg-rust |
| Arrow | `arrow` | 54+ | ใช้ร่วมกับ DataFusion |
| Parquet | `parquet` | 54+ | ใช้ร่วมกับ DataFusion |
| AWS SDK (optional) | `aws-config` + `aws-sdk-s3` | latest | สำหรับ presigned URLs / advanced |
| Azure SDK (optional) | `azure_storage` | latest | สำหรับ Managed Identity |

---

## 6. Project Structure (Phase 2 additions)

```
src/
├── domain/
│   └── ports/
│       └── warehouse_client.rs   # ← ไม่เปลี่ยน! Phase 2 implement trait เดิม
│
└── infrastructure/
    ├── http_client/
    │   ├── databricks_client.rs  # Phase 1 ✅
    │   └── retry.rs              # Phase 1 ✅
    │
    ├── datafusion/               # NEW — Phase 2
    │   ├── mod.rs
    │   ├── engine.rs             # DataFusion SessionContext factory
    │   ├── client.rs             # DataFusionClient impl WarehouseClient
    │   ├── statement_store.rs    # In-memory statement tracking
    │   ├── result_converter.rs   # RecordBatch → StatementResult
    │   └── type_mapper.rs        # Arrow types → Databricks types
    │
    ├── object_store/             # NEW — Phase 2
    │   ├── mod.rs
    │   ├── s3.rs                 # S3 ObjectStore factory
    │   ├── azure_blob.rs         # Azure Blob ObjectStore factory
    │   └── local.rs              # Local filesystem (dev only)
    │
    ├── catalog/                  # NEW — Phase 2
    │   ├── mod.rs
    │   ├── static_catalog.rs     # Load tables from config.toml
    │   ├── delta_table.rs        # Register Delta Lake tables
    │   ├── iceberg_table.rs      # Register Iceberg tables
    │   └── parquet_table.rs      # Register Parquet tables
    │
    ├── config/
    │   └── settings.rs           # ADD: catalog settings, datafusion settings
    │
    └── routing/                  # NEW — Phase 2
        └── backend_router.rs     # warehouse_id → DatabricksClient | DataFusionClient
```

---

## 7. Configuration Schema (Phase 2)

```toml
# ── Server (unchanged) ────────────────────────────────────────────────
[server]
host = "0.0.0.0"
port = 8360

# ── Upstream routing ────────────────────────────────────────────────
[upstream]
default_warehouse_id = "wh-s3-prod"

# Databricks backend (Phase 1 style — unchanged)
[[upstream.warehouses]]
id           = "wh-databricks"
backend_type = "databricks"
host         = "adb-xxxx.azuredatabricks.net"
http_path    = "/sql/1.0/warehouses/abc"
token_env    = "DATABRICKS_TOKEN"

# DataFusion backend (Phase 2)
[[upstream.warehouses]]
id           = "wh-s3-prod"
backend_type = "datafusion"
catalog      = "prod_catalog"
query_timeout_secs = 300
result_ttl_secs    = 3600

[[upstream.warehouses]]
id           = "wh-azure-analytics"
backend_type = "datafusion"
catalog      = "analytics_catalog"

# ── Object Storage credentials ────────────────────────────────────────
[object_storage.s3]
region           = "ap-southeast-1"
# access_key_id and secret_access_key via env: AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY
# (omit to use IAM Role)

[object_storage.azure]
account_name = "myaccount"
# account_key via env: AZURE_STORAGE_ACCOUNT_KEY
# (omit to use Managed Identity)

# ── Catalogs ─────────────────────────────────────────────────────────
[catalogs.prod_catalog]
description = "Production S3 data lake"

[[catalogs.prod_catalog.tables]]
name     = "orders"
format   = "delta"
location = "s3://prod-bucket/delta/orders/"

[[catalogs.prod_catalog.tables]]
name     = "products"
format   = "parquet"
location = "s3://prod-bucket/parquet/products/"
partition_columns = ["category"]

[catalogs.analytics_catalog]
description = "Azure analytics data"

[[catalogs.analytics_catalog.tables]]
name     = "page_views"
format   = "parquet"
location = "az://analytics-container/parquet/page_views/"

# ── DataFusion engine tuning ─────────────────────────────────────────
[datafusion]
max_concurrent_queries  = 20
target_partitions       = 8      # parallelism per query
memory_limit_mb         = 4096   # per query memory limit
enable_parquet_pruning  = true
enable_predicate_pushdown = true
```

---

## 8. Acceptance Criteria (Phase 2 Complete)

- [ ] `cargo build --release` สำเร็จโดยไม่มี warning (รวม DataFusion dependencies)
- [ ] Submit query ไปยัง DataFusion warehouse และรับผลลัพธ์กลับ (JSON_ARRAY)
- [ ] อ่าน Parquet file จาก S3 และ return ผลลัพธ์ถูกต้อง
- [ ] อ่าน Delta Lake table จาก S3 และ return ผลลัพธ์ถูกต้อง
- [ ] อ่าน Azure Blob Storage (Parquet) และ return ผลลัพธ์ถูกต้อง
- [ ] Multi-backend routing — request ไปยัง `wh-databricks` ไปถึง Databricks, `wh-s3-prod` ไปถึง DataFusion
- [ ] Async polling (submit → RUNNING → SUCCEEDED)
- [ ] Cancel DataFusion query ระหว่าง execution
- [ ] Unit tests สำหรับ DataFusion components ผ่านทั้งหมด
- [ ] Integration tests ด้วย local filesystem backend (`file://`) ผ่านทั้งหมด
- [ ] Docker image ยังคงขนาดไม่เกิน 100MB

---

## 9. Milestones

| Milestone | งาน | ระยะเวลาประมาณ |
|-----------|-----|----------------|
| M1 | DataFusion session factory + in-memory statement store | 3–4 วัน |
| M2 | Object store factory: S3 + Azure Blob + Local | 3–4 วัน |
| M3 | Static catalog loader — Parquet tables | 2–3 วัน |
| M4 | Delta Lake integration (`deltalake` crate) | 3–5 วัน |
| M5 | `DataFusionClient` impl `WarehouseClient` + result converter | 4–5 วัน |
| M6 | Backend router — warehouse_id → Databricks / DataFusion | 2–3 วัน |
| M7 | Iceberg support (`iceberg-rust`) | 3–4 วัน |
| M8 | ARROW_STREAM result format | 2–3 วัน |
| M9 | Tests, docs, performance tuning | 3–4 วัน |
| **Total** | | **~4–6 สัปดาห์** |

---

## 10. Dependencies & Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| `deltalake` crate API changes frequently | Medium | Pin exact version, read changelog carefully |
| DataFusion memory usage ยาก predict | Medium | ตั้ง per-query memory limit, test with representative data |
| S3 latency สูงสำหรับ interactive queries | Medium | ใช้ Parquet column pruning + predicate pushdown ให้เต็มที่ |
| `iceberg-rust` ยังอยู่ใน early stage | High | Implement Iceberg เป็น optional feature ก่อน |
| Arrow/Parquet/DataFusion version alignment | Medium | ใช้ DataFusion's re-exported `arrow` + `parquet` crates แทน import ตรง |

---

*Document Version: 1.0 | Created: 2026-03-06 | สร้างโดย: Claude (Cowork mode)*
