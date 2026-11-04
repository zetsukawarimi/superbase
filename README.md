# ⚡ Superbase: Ultra-Fast Native JSON Document Database in 100% Pure Rust

Superbase adalah database dokumen JSON berperforma ultra-tinggi yang dirancang dari nol menggunakan **100% Safe & Concurrent Rust**. 

Superbase dirancang khusus untuk memecahkan kelemahan fundamental arsitektur RDBMS tradisional seperti **MySQL / MariaDB** saat menangani data berbasis JSON.

---

## 🥊 Mengapa Superbase Jauh Lebih Bagus dari MySQL untuk JSON?

| Fitur / Karakteristik | 🐬 MySQL (InnoDB + JSON) | ⚡ Superbase (Full Rust) |
|---|---|---|
| **Penyimpanan Dokumen** | JSON disimpan sebagai BLOB di dalam baris B-Tree. Update field kecil memicu rewrite seluruh baris. | **Log-Structured Append-Only (WAL)**. Tulis berurutan (sequential append) O(1) tanpa rewrite. |
| **Kecepatan Write** | ~10k – 20k writes/detik (Bottleneck di buffer pool, mutex, undo/redo log). | **150,000+ writes/detik** (Zero locks, non-blocking asynchronous green-threads). |
| **Secondary Indexing pada JSON** | Wajib deklarasi `ALTER TABLE ... ADD COLUMN ... GENERATED ALWAYS AS (JSON_EXTRACT(...))` lalu tambah indeks. Rumit & mengunci tabel. | **Dynamic First-Class Indexing**. Cukup panggil `create_index("profile.age")` secara dinamis kapan saja. |
| **Multikey Indexing (Array)** | Terbatas pada `JSON_TABLE` atau multi-value index terbatas di versi terbaru. | **Dukungan Penuh Multikey B-Tree**. Tiap elemen array diindeks secara otomatis (`tags: ["rust", "database"]`). |
| **Query Engine** | Sintaks SQL berbelit (`JSON_UNQUOTE(JSON_EXTRACT(data, '$.age')) >= 21`). | **Native JSON Filter DSL** (`{"age": {"$gte": 21}}`) mirip MongoDB tapi ditulis dalam Rust murni tanpa JVM/Go GC pause. |
| **Model Threading** | 1 thread per koneksi OS (berat & rentan contention). | **Tokio Work-Stealing Runtime** (mampu menangani jutaan concurrent connections dengan memori minimal). |
| **In-Memory Cache & Primary Key** | Buffer Pool cache berbasis Page Table. | **DashMap Concurrent Lock-Free O(1)** (sub-mikrodetik direct lookup). |
| **Integritas & Crash-Recovery** | Doublewrite buffer + redo log. | **CRC32 Checksummed WAL Records** dengan replikasi instan saat recovery. |

---

## 🏛️ Arsitektur Superbase

```
                  ┌───────────────────────────────────────────────┐
                  │   Client Requests (HTTP REST / CLI / REPL)    │
                  └───────────────────────┬───────────────────────┘
                                          │
                                   [ Axum + Tokio ]
                                          │
                                          ▼
                   ┌─────────────────────────────────────────────┐
                   │               Collection API                │
                   └───────┬─────────────────────────────┬───────┘
                           │                             │
                ┌──────────▼──────────┐       ┌──────────▼──────────┐
                │ Primary Key Index   │       │ Secondary Index     │
                │ (Concurrent DashMap)│       │ (Concurrent B-Tree) │
                └──────────┬──────────┘       └──────────┬──────────┘
                           │                             │
                           └──────────────┬──────────────┘
                                          │
                                          ▼
                   ┌─────────────────────────────────────────────┐
                   │           Write-Ahead Log (WAL)             │
                   │      [ Header | CRC32 | Key | JSON ]        │
                   │  Append-Only Segments + Background Compact  │
                   └─────────────────────────────────────────────┘
```

---

## 🚀 Panduan Memulai Cepat

### 1. Menjalankan Server Database
```bash
cargo run --release --bin superbase-server -- --port 8080 --data-dir ./my_data
```

### 2. Menjalankan Superbase Studio (GUI Management Software)
Software GUI interaktif, ringan, dan lengkap dalam 100% Rust untuk kontrol dan manajemen data:
```bash
cargo run --release --bin superbase-studio -- --port 9090 --data-dir ./my_data
```
*Browser default Anda akan otomatis terbuka menuju `http://127.0.0.1:9090`.*

### 3. Universal Database Migration Engine (CLI & GUI)
Superbase dilengkapi dengan mesin migrasi dua arah (bidirectional) untuk semua jenis database:
```bash
# Export Superbase ke script MySQL SQL Dump
cargo run --release --bin superbase-migrate -- export --collection users --format mysql --output users_mysql.sql

# Export Superbase ke PostgreSQL (native JSONB & GIN Index)
cargo run --release --bin superbase-migrate -- export --collection users --format postgres --output users_pg.sql

# Export Superbase ke format MongoDB (mongoimport NDJSON)
cargo run --release --bin superbase-migrate -- export --collection users --format mongo --output users_mongo.ndjson

# Export Superbase ke CSV Spreadsheet
cargo run --release --bin superbase-migrate -- export --collection users --format csv --output users.csv

# Import dari MySQL dump ke dalam Superbase
cargo run --release --bin superbase-migrate -- import --collection users --format mysql --input users_mysql.sql

# Import dari CSV ke dalam Superbase
cargo run --release --bin superbase-migrate -- import --collection users --format csv --input users.csv
```
*(Fitur migrasi ini juga tersedia langsung secara visual melalui tombol **🔄 Migration Hub** di dalam Superbase Studio GUI).*

### 4. Menggunakan CLI & Interactive REPL
```bash
cargo run --release --bin superbase-cli -- repl
```

### 5. Menjalankan Benchmark Engine
```bash
cargo run --release --bin superbase-bench -- --count 50000 --concurrency 8
```

---

## 📡 REST API Reference

### Health & Stats
* `GET /health` – Status server dan engine
* `GET /api/v1/stats` – Statistik koleksi dan penggunaan

### Koleksi
* `GET /api/v1/collections` – Daftar koleksi
* `POST /api/v1/collections` – Buat koleksi baru `{ "name": "users" }`
* `DELETE /api/v1/collections/{collection}` – Hapus koleksi

### Dokumen (CRUD)
* `POST /api/v1/collections/{collection}/documents` – Tambah dokumen JSON (otomatis diberi `_id`, `_version`, `_created_at`)
* `GET /api/v1/collections/{collection}/documents/{id}` – Ambil dokumen berdasarkan ID
* `PUT /api/v1/collections/{collection}/documents/{id}` – Perbarui dokumen
* `DELETE /api/v1/collections/{collection}/documents/{id}` – Hapus dokumen

### Query Dinamis
`POST /api/v1/collections/{collection}/query`
Contoh Payload:
```json
{
  "filter": {
    "profile.age": { "$gte": 21, "$lte": 50 },
    "role": "admin",
    "tags": { "$contains": "rust" }
  },
  "sort": {
    "profile.age": "desc"
  },
  "skip": 0,
  "limit": 20,
  "project": ["_id", "username", "profile.age"]
}
```

### Dynamic Secondary Indexing
* `POST /api/v1/collections/{collection}/indexes` – Buat indeks pada field JSON bertingkat:
  ```json
  { "path": "profile.age" }
  ```
* `GET /api/v1/collections/{collection}/indexes` – Lihat indeks aktif
* `POST /api/v1/collections/{collection}/compact` – Jalankan kompresi segment & garbage collection

---

## 💻 Penggunaan sebagai Embedded Library di Rust

Anda juga dapat menggunakan Superbase secara langsung di dalam kode Rust Anda tanpa server:

```rust
use superbase::{Database, QueryRequest, QueryFilter};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open("./data", None)?;
    let users = db.collection("users")?;

    // Buat index dinamis
    users.create_index("role")?;

    // Simpan dokumen
    let doc = json!({
        "name": "Budi",
        "role": "developer",
        "skills": ["rust", "actix", "tokio"]
    });
    let inserted = users.insert(doc)?;
    println!("Inserted doc with ID: {}", inserted["_id"]);

    // Query dokumen
    let results = users.query(QueryRequest::default())?;
    println!("Found {} documents", results.len());

    Ok(())
}
```
