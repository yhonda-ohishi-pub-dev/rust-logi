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
- `hono-api-test_ref/` - 参照用: hono-api-test リポジトリ (https://github.com/yhonda-ohishi/hono-api-test.git)

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

## 現在の作業: DVR Notifications 機能 (完了)

### 背景
browser-render から DVR 通知データを受け取り、PostgreSQL に保存し、LINE WORKS に通知を送る機能。

### 完了
- [x] `packages/logi-proto/proto/dvr_notifications.proto` 作成（BulkCreate RPC）
- [x] `migrations/00015_create_dvr_notifications.sql` 作成・適用
- [x] `src/services/dvr_notifications_service.rs` 実装（mp4_url重複チェック + LINE通知）
- [x] `src/config.rs` に DVR 環境変数追加
- [x] `.env` に DVR 環境変数追加
- [x] rust-logi を Cloud Run にデプロイ

### 環境変数
```bash
DVR_NOTIFICATION_ENABLED=true
DVR_LINEWORKS_BOT_URL=https://lineworks-bot-rust-566bls5vfq-an.a.run.app
```

### デプロイ済みURL
- rust-logi: `https://rust-logi-747065218280.asia-northeast1.run.app`
- lineworks-bot-rust: `https://lineworks-bot-rust-566bls5vfq-an.a.run.app`

### 関連ファイル
- `packages/logi-proto/proto/dvr_notifications.proto` - Proto定義
- `src/services/dvr_notifications_service.rs` - サービス実装
- `src/models/dvr_notification.rs` - モデル
- `migrations/00015_create_dvr_notifications.sql` - マイグレーション
- `lineworks-bot-rust_ref/` - LINE WORKS Bot参照

---

## 次の作業: browser-render-rust → rust-logi DVR通知連携

### 未完了
1. **browser-render-rust に DVR通知 gRPC クライアント追加** - rust-logi の DvrNotificationsService.BulkCreate を呼び出す
2. **browser-render-rust のビルド・テスト**
3. **browser-render-rust のデプロイ（GCE）**

### 次のアクション
```bash
# browser-render-rust_ref でDVR通知クライアント実装
# 1. build.rs に dvr_notifications.proto 追加
# 2. renderer.rs に send_dvr_to_rust_logi メソッド追加
# 3. DVRイベント発生時に呼び出し

cd browser-render-rust_ref && cargo build --features grpc
```

---

## 過去の作業: browser-render-rust → rust-logi Dtakologs統合 (完了)

### 完了
- [x] Phase 1: `packages/logi-proto/proto/dtakologs.proto`に`BulkCreate` RPC追加
- [x] Phase 1: `src/services/dtakologs_service.rs`に`bulk_create`実装
- [x] Phase 2: `browser-render-rust_ref/build.rs`にlogiプロトコンパイル追加
- [x] Phase 2: `browser-render-rust_ref/src/lib.rs`にlogiモジュール追加
- [x] Phase 2: `browser-render-rust_ref/src/config.rs`に`rust_logi_url`, `rust_logi_organization_id`追加
- [x] Phase 2: `browser-render-rust_ref/src/browser/renderer.rs`に`send_to_rust_logi`メソッド追加

### 参照リポジトリ
- `browser-render-rust_ref/` - https://github.com/yhonda-ohishi-pub-dev/browser-render-rust.git
- `browser_render_go_ref/` - https://github.com/yhonda-ohishi/browser_render_go.git
- `hono-api-test_ref/` - https://github.com/yhonda-ohishi/hono-api-test.git
- `lineworks-bot-rust_ref/` - https://github.com/yhonda-ohishi-pub-dev/lineworks-bot-rust.git

### 注意事項
- `RUST_LOGI_URL`と`RUST_LOGI_ORGANIZATION_ID`は必須（デフォルト値なし）
- browser-render-rustはGCEのまま運用（Chrome安定性のため）
- grpc featureが必要（`--features grpc`）
