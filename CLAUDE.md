# rust-logi

hono-logiのRust実装（gRPC-Web対応）

## 最新のhandover

新しいセッション開始時は必ずここに記載されたファイルを読んで前回の状況を把握すること。
handoverの全タスクが完了したら `handover/completed/` に移動し、ここのパスを削除すること。

(現在アクティブな引き継ぎなし)

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

- `migrations/` - PostgreSQLマイグレーション (00001-00018)
- `src/db/organization.rs` - RLSヘルパー関数 (`set_current_organization`, `get_current_organization`)
- `src/storage/mod.rs` - GCSクライアント
- `packages/logi-proto/` - npmパッケージ（proto + 生成済みTypeScript）
- `docs/` - 設計ドキュメント
  - `car-inspection-date-fields.md` - 車検証日付フィールドの仕様（スペース含む値の扱い）
  - `file-car-inspection-linkage.md` - ファイルと車検証の紐づき構造（テーブル関係、再アップロード時の挙動）
- `convert_and_import.py` - hono-logiからのデータ移行スクリプト
- `.env` - 環境変数 (DATABASE_URL等)
- `rust-logi.code-workspace` - VSCodeマルチルートワークスペース（全参照リポを含む）
- `front/` - フロントエンド参照（symlink）
  - `nuxt-pwa-carins/` → /home/yhonda/js/nuxt-pwa-carins（メインUI）
  - `nuxt-dtako-logs/` → /home/yhonda/js/nuxt_dtako_logs（DTakoログビューワー）
- `workers/` - Cloudflare Workers参照（symlink）
  - `cf-grpc-proxy/` → /home/yhonda/js/nuxt_dtako_logs/cf-grpc-proxy（gRPCプロキシ）
  - `smb-upload-worker/` → /home/yhonda/js/smb-upload-worker（SMBアップロード）
  - `auth-worker/` → /home/yhonda/js/auth-worker（JWT認証）
- `services/` - バックエンドサービス参照（symlink）
  - `browser-render-rust/` → /home/yhonda/rust/browser-render_rust（DVRレンダリング）
    - `rust-scraper/` - git submodule（車両データスクレイピング）
      - `data/` - スクレイピング結果JSON（gitignore）
  - `lineworks-bot-rust/` → /home/yhonda/rust/lineworks-bot-rust（LINE WORKS Bot）
  - `smb-watch/` → /home/yhonda/rust/smb-watch（SMB監視・ファイルアップロード）
- `legacy/` - レガシー参照
  - `hono-logi/` → /home/yhonda/js/hono-logi（旧Cloudflare Workers版）
  - `hono-api-test/` — 実体ディレクトリ
  - `browser-render-go/` — 実体ディレクトリ

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

## 進行中: PDF/JSON紐づき修正 — organization_id不整合 + uuid型バグ

### 原因（特定済み）
データが2つのorganization_idに分裂していた：
- `00000000-0000-0000-0000-000000000001` (Default) — dtakologs, kudg系, car_inspection等
- `01926fa0-20fe-70bd-8b60-9d268f28a987` (Temporary) — files系テーブルのみ

アプリはデフォルト`00000000-...0001`を使うため、車検証は見えるがファイル紐づきが0件だった。

### 完了（DB修正）
- [x] ファイル系4テーブルのorganization_idを `00000000-...0001` に統一
  - files: 2,714件 UPDATE済み
  - car_inspection_files_a: 1,380件 UPDATE済み
  - car_inspection_files_b: 710件 UPDATE済み
  - car_inspection_files: 667件 UPDATE済み
- [x] car_inspection の重複レコード削除（`01926fa0-...a987` の205件）
- [x] ichiban_cars の重複レコード削除（`01926fa0-...a987` の509件）
- [x] Temporary Organization レコード削除

### 完了（コード修正・未デプロイ）
- [x] `CarInspectionFileModel.uuid` の型を `String` → `uuid::Uuid` に修正
  - `src/models/car_inspection_files.rs:6` — `pub uuid: uuid::Uuid,`
  - `src/services/car_inspection_service.rs:982` — `uuid: model.uuid.to_string(),`
  - DBのuuidカラムはUUID型だがRustでStringとして読んでいた（RLSで0件だったので顕在化せず）
- [x] `cargo build --release` 成功確認済み

### 未完了
1. **Cloud Runにデプロイ** — uuid型バグ修正を反映するため必須
   ```bash
   # デプロイスクリプト使用
   ./deploy.sh
   ```
2. **API検証** — デプロイ後に以下を確認:
   ```bash
   TOKEN=$(gcloud auth print-identity-token)
   # pdf_uuid / json_uuid が返ることを確認
   grpcurl -H "Authorization: Bearer $TOKEN" \
     -H "x-organization-id: 00000000-0000-0000-0000-000000000001" \
     -d '{}' rust-logi-747065218280.asia-northeast1.run.app:443 \
     logi.car_inspection.CarInspectionService/ListCurrentCarInspections 2>&1 | grep -E "pdfUuid|jsonUuid"

   # ListCurrentCarInspectionFiles が空でないことを確認
   grpcurl -H "Authorization: Bearer $TOKEN" \
     -H "x-organization-id: 00000000-0000-0000-0000-000000000001" \
     -d '{}' rust-logi-747065218280.asia-northeast1.run.app:443 \
     logi.car_inspection.CarInspectionFilesService/ListCurrentCarInspectionFiles
   ```
3. **フロントエンド（nuxt-pwa-carins）からPDF/JSONの紐づき表示確認**

### 注意事項
- Cloud Run上の現バイナリには uuid型バグがあるため、デプロイ前はListCurrentCarInspectionFilesがエラーになる
- ListCurrentCarInspectionsのpdf_uuid/json_uuidサブクエリは `uuid::text` キャストを使っているのでデプロイ後は動くはず
- `debug_org_ids()` 関数がDBに残っている（不要なら `DROP FUNCTION debug_org_ids();` で削除可）

---

## 完了: 全gRPCメソッド FORCE RLS対応 (revision: rust-logi-00054-rsd)

### 背景
`migrations/00018_force_rls_on_all_tables.sql` で全28テーブルに `FORCE ROW LEVEL SECURITY` を適用後、`set_current_organization()` を呼ばないメソッドのクエリが全て0件を返していた。PDF/JSONの紐づきが0件になる問題の直接原因。

### 修正内容（22メソッド）
全メソッドに以下のパターンを追加:
```rust
let organization_id = get_organization_from_request(&request);
let mut conn = self.pool.acquire().await...;
set_current_organization(&mut conn, &organization_id).await...;
// クエリは &mut *conn で実行
```

#### car_inspection_service.rs (9メソッド)
- [x] `create_car_inspection` — `current_setting` 使用のため事前に `set_current_organization` 必須
- [x] `list_car_inspections`
- [x] `get_car_inspection`
- [x] `delete_car_inspection`
- [x] `list_expired_or_about_to_expire`
- [x] `list_renew_targets`
- [x] `create_car_inspection_file` — `current_setting` 使用のため事前に `set_current_organization` 必須
- [x] `list_car_inspection_files`
- [x] `list_current_car_inspection_files`

#### files_service.rs (8メソッド)
- [x] `create_file` — org_id取得済みだったが `set_current_organization` 未呼出だった
- [x] `list_files`
- [x] `get_file`
- [x] `download_file` — org_id取得済みだったが `set_current_organization` 未呼出だった
- [x] `delete_file`
- [x] `list_not_attached_files`
- [x] `list_recent_uploaded_files`
- [x] `restore_file`

#### cam_files_service.rs (5メソッド)
- [x] `list_cam_files`
- [x] `list_cam_file_dates`
- [x] `create_cam_file_exe`
- [x] `list_stages`
- [x] `create_stage`

#### dvr_notifications_service.rs (1メソッド)
- [x] `retry_pending_downloads`

### 対応済みだったメソッド（修正不要）
- `list_current_car_inspections` (car_inspection_service.rs)
- `list_renew_home_targets` (car_inspection_service.rs)
- DtakologsService 全9メソッド
- FlickrService 全2メソッド
- DvrNotificationsService `bulk_create`
- HealthService (DB未使用)

### RLS必須ルール
新しいgRPCメソッドを追加する際は **必ず** `set_current_organization()` を呼ぶこと。呼ばないと全テーブルで0件が返る。

### デプロイ
- Cloud Run revision: `rust-logi-00054-rsd`
- Health check: SERVING

---

## 完了: DVR Notifications 機能

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
- `services/lineworks-bot-rust/` - LINE WORKS Bot参照

---

## 次の作業: DVR通知 mp4 → GCS 保存機能

### 概要
DVR通知受信時にmp4ファイルを外部URLからダウンロードし、GCSに保存する。

### 計画書
`/home/yhonda/.claude/plans/keen-gliding-catmull.md`

### 実装予定
1. **マイグレーション作成**: `migrations/00016_add_dvr_mp4_storage.sql`
   - `gcs_key TEXT`
   - `file_size_bytes BIGINT`
   - `download_status VARCHAR(20) DEFAULT 'pending'`

2. **DVR通知サービス拡張**: `src/services/dvr_notifications_service.rs`
   - `download_and_store_mp4()` 追加
   - `bulk_create` 内で `tokio::spawn` で非同期ダウンロード

3. **モデル更新**: `src/models/dvr_notification.rs`
   - 新カラム追加

### データフロー
```
DVR通知受信 → DB保存(pending) → LINE通知 → tokio::spawn
                                              ↓
                                    mp4ダウンロード(HTTP)
                                              ↓
                                    GCSアップロード
                                              ↓
                                    DB更新(completed)
```

### GCS保存パス
`gs://rust-logi-files/{org_id}/dvr/{uuid}.mp4`

---

## 完了: browser-render-rust → rust-logi Dtakologs統合

### commit
`467c6db` - Add rust-logi gRPC integration for direct PostgreSQL data insertion

### 確認済み
- [x] DVR通知 5件登録済み（堺100あ5850 急加速など）
- [x] GCS 2,714ファイル / 128.48 MiB
- [x] DB files テーブル 2,714件（GCSと一致）

---

## 過去の作業: browser-render-rust → rust-logi Dtakologs統合 (完了)

### 完了
- [x] Phase 1: `packages/logi-proto/proto/dtakologs.proto`に`BulkCreate` RPC追加
- [x] Phase 1: `src/services/dtakologs_service.rs`に`bulk_create`実装
- [x] Phase 2: `services/browser-render-rust/build.rs`にlogiプロトコンパイル追加
- [x] Phase 2: `services/browser-render-rust/src/lib.rs`にlogiモジュール追加
- [x] Phase 2: `services/browser-render-rust/src/config.rs`に`rust_logi_url`, `rust_logi_organization_id`追加
- [x] Phase 2: `services/browser-render-rust/src/browser/renderer.rs`に`send_to_rust_logi`メソッド追加

### 参照リポジトリ
- `services/browser-render-rust/` - https://github.com/yhonda-ohishi-pub-dev/browser-render-rust.git
- `legacy/browser-render-go/` - https://github.com/yhonda-ohishi/browser_render_go.git
- `legacy/hono-api-test/` - https://github.com/yhonda-ohishi/hono-api-test.git
- `legacy/hono-logi/` - 元のhono-logi実装（Cloudflare Workers版）
- `services/lineworks-bot-rust/` - https://github.com/yhonda-ohishi-pub-dev/lineworks-bot-rust.git
- `front/nuxt-pwa-carins/` - /home/yhonda/js/nuxt-pwa-carins（フロントエンド）

### 注意事項
- `RUST_LOGI_URL`と`RUST_LOGI_ORGANIZATION_ID`は必須（デフォルト値なし）
- browser-render-rustはGCEのまま運用（Chrome安定性のため）
- grpc featureが必要（`--features grpc`）
