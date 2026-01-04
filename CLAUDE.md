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
- `packages/logi-proto/` - npmパッケージ（proto + 生成済みTypeScript）
- `docs/` - 設計ドキュメント
  - `car-inspection-date-fields.md` - 車検証日付フィールドの仕様（スペース含む値の扱い）
- `convert_and_import.py` - hono-logiからのデータ移行スクリプト
- `.env` - 環境変数 (DATABASE_URL等)

## データ移行 (convert_and_import.py)

hono-logiのpg_dumpからrust-logiにデータを移行するスクリプト。

```bash
python3 convert_and_import.py
```

### 処理内容

1. pg_dumpのSQLファイルを解析
2. organization_idを追加してマルチテナント対応
3. car_inspectionの重複レコード（スペース違い）をスキップ
4. car_inspection_files_a/bのGrantdateにスペースを追加

詳細は `docs/car-inspection-date-fields.md` を参照。

## npmパッケージ (@yhonda-ohishi-pub-dev/logi-proto)

protoファイルと生成済みTypeScriptを含むnpmパッケージ。GitHub Packagesで公開。

### インストール

```bash
# .npmrcに追加
echo "@yhonda-ohishi-pub-dev:registry=https://npm.pkg.github.com" >> .npmrc

# インストール
npm install @yhonda-ohishi-pub-dev/logi-proto
```

### 使い方

```typescript
import { File, FilesService } from "@yhonda-ohishi-pub-dev/logi-proto";
import { createClient } from "@connectrpc/connect";
import { createGrpcWebTransport } from "@connectrpc/connect-web";

const transport = createGrpcWebTransport({ baseUrl: "http://localhost:50051" });
const client = createClient(FilesService, transport);
```

### pre-pushフック

`git push`時に自動でTypeScript生成とGitHub Packagesへの公開が実行される。

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
