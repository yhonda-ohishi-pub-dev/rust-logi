# rust-logi

hono-logiのRust実装（gRPC-Web対応）

## データベース

- **DB**: Cloud SQL PostgreSQL (`cloudsql-sv:asia-northeast1:postgres-prod`)
- **データベース名**: `rust_logi_test`
- **マルチテナント**: RLS (Row Level Security) + `organization_id` カラム

### マイグレーション管理

sqlxが`_sqlx_migrations`テーブルで管理。状態確認:
```bash
PGPASSWORD=kikuraku psql -h 127.0.0.1 -p 5432 -U postgres -d rust_logi_test \
  -c "SELECT version, description, installed_on FROM _sqlx_migrations ORDER BY version;"
```

### マイグレーション実行

```bash
sqlx migrate run --database-url "postgres://postgres:kikuraku@127.0.0.1:5432/rust_logi_test"
```

または`.env`を読み込んで:
```bash
source .env && sqlx migrate run
```

## 起動方法

```bash
# ターミナル1: Cloud SQL Proxy起動（なければ自動ダウンロード）
./start-proxy.sh

# ターミナル2: サーバー起動
./start.sh
```

## プロジェクト構成

- `migrations/` - PostgreSQLマイグレーション (00001-00008)
- `src/db/organization.rs` - RLSヘルパー関数 (`set_current_organization`, `get_current_organization`)
- `src/storage/mod.rs` - GCSクライアント
- `.env` - 環境変数 (DATABASE_URL等)

## ファイルストレージ

### 構成

- **ストレージ**: GCS (Google Cloud Storage) - Autoclass有効
- **DB**: メタデータのみ保存（`files`テーブルの`blob`カラムはNULL）

### データフロー

```
クライアント (Base64) → gRPC-Web → Rust (デコード) → GCS (バイナリ保存)
```

- gRPC-Web/JSONではバイナリ送信にBase64が必要
- Rustでデコード後、GCSにはバイナリで保存
- DBにはパス（`s3_key`）とメタデータのみ

### filesテーブル

| カラム | 用途 |
|--------|------|
| `uuid` | ファイルID |
| `s3_key` | GCSパス (`{org_id}/{uuid}`) |
| `storage_class` | STANDARD等（Autoclassで自動管理） |
| `blob` | 未使用（NULL） |
| `access_count_*` | アクセス統計 |

### コスト比較

| ストレージ | 料金 |
|------------|------|
| Cloud SQL | $0.17/GB/月 |
| GCS Standard | $0.023/GB/月 |
| GCS Coldline | $0.004/GB/月 |

PDFなどの大きいファイルはGCSに保存することでコスト削減。
