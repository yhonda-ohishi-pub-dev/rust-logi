# rust-logi

hono-logiのRust実装（gRPC-Web対応）

## 最新のhandover

新しいセッション開始時は必ずここに記載されたファイルを読んで前回の状況を把握すること。
handoverの全タスクが完了したら `handover/completed/` に移動し、ここのパスを削除すること。

- `handover/2026-02-25_05-00.md` — Phase 2 ストレージ抽象化 GCS/R2 両対応（実装完了・デプロイ前）
- `handover/2026-02-25_03-20.md` — Backend 移行 Supabase/CF Containers（Phase 0 完了・Phase 1 Cloud Run デプロイ待ち）
- `handover/2026-02-23_19-30.md` — Items 詳細表示 + 所有権変更（デプロイ済み・「個人に移動」ボタン動作しない問題あり）
- `handover/2026-02-23_17-50.md` — Amazon 共有 URL → PWA 物品登録（デプロイ済み・E2E確認待ち・extractNameFromShareText 修正必要）
- `handover/2026-02-22_17-08.md` — 組織slug表示完了 + 複数組織対応（次回タスク）
- `handover/2026-02-22_10-30.md` — nuxt-items UI調整完了（全完了）
- `handover/2026-02-22_07-05.md` — Items 機能（DB・バックエンド完了・フロントエンドデプロイ済み）
- `handover/2026-02-22_06-19.md` — JWT org_slug 追加（Phase 1 完了・デプロイ済み・手動検証待ち）
- `handover/2026-02-21_18-34.md` — LINE WORKS Bot リッチメニュー管理画面（DB/Proto/Rust完了・auth-worker API ハンドラ実装途中）
- `handover/2026-02-21_17-23.md` — logout 後 WOFF login できない問題（cookie Domain 修正デプロイ済み / DB ドメイン不一致 + stale cookie が残る問題が未解決）
- `handover/2026-02-21_16-52.md` — WOFF トップページ + cross-subdomain cookie 共有（全デプロイ済み / Developer Console 手動設定のみ残り）

## データベース

### Cloud SQL（既存・フォールバック用）
- **DB**: Cloud SQL PostgreSQL (`cloudsql-sv:asia-northeast1:postgres-prod`)
- **データベース名**: `rust_logi_test`
- **接続**: `postgres://postgres:kikuraku@127.0.0.1:5432/rust_logi_test`（Cloud SQL Proxy 経由）

### Supabase（移行先・東京リージョン）
- **プロジェクト**: `https://tvbjvhvslgdwwlhpkezh.supabase.co`
- **リージョン**: Northeast Asia (Tokyo) ap-northeast-1
- **データベース名**: `postgres`
- **接続**: `postgresql://rust_logi_app:xxx@db.tvbjvhvslgdwwlhpkezh.supabase.co:5432/postgres`

#### Supabase 接続の重要事項
- **`rust_logi_app` ユーザーで接続すること**（NOBYPASSRLS）
- Supabase の `postgres` ユーザーは `BYPASSRLS=true` のため **RLS が効かない**
- `rust_logi_app` は RLS 対象なので `set_current_organization()` が正しく動作する
- **Supavisor transaction mode (port 6543) は使用不可** — `set_config` がリセットされる
- 必ず **直接接続 (port 5432)** を使用すること

### マルチテナント
- RLS (Row Level Security) + `organization_id` カラム
- 28テーブルに FORCE ROW LEVEL SECURITY 適用

### マイグレーション管理

sqlxが`_sqlx_migrations`テーブルで管理。状態確認:
```bash
# Cloud SQL
PGPASSWORD=kikuraku psql -h 127.0.0.1 -p 5432 -U postgres -d rust_logi_test \
  -c "SELECT version, description, installed_on FROM _sqlx_migrations ORDER BY version;"

# Supabase
PGPASSWORD=Zo6hYIWs7yH0sTah psql -h db.tvbjvhvslgdwwlhpkezh.supabase.co -p 5432 -U postgres -d postgres \
  -c "SELECT version, description, installed_on FROM _sqlx_migrations ORDER BY version;"
```

### マイグレーション実行

```bash
# Cloud SQL
sqlx migrate run --database-url "postgres://postgres:kikuraku@127.0.0.1:5432/rust_logi_test"

# Supabase（postgres ユーザーで実行 — DDL には BYPASSRLS が必要）
sqlx migrate run --database-url "postgresql://postgres:Zo6hYIWs7yH0sTah@db.tvbjvhvslgdwwlhpkezh.supabase.co:5432/postgres"
```

## 起動方法

```bash
# ターミナル1: Cloud SQL Proxy起動（Cloud SQL 使用時のみ）
./start-proxy.sh

# ターミナル2: サーバー起動（Cloud SQL）
./start.sh

# Supabase で起動する場合
DATABASE_URL="postgresql://rust_logi_app:Zo6hYIWs7yH0sTah@db.tvbjvhvslgdwwlhpkezh.supabase.co:5432/postgres" cargo run --release
```

## デプロイ

```bash
# rust-logi (Cloud Run)
./deploy.sh

# auth-worker (Cloudflare Workers) — git push で自動デプロイ
cd /home/yhonda/js/auth-worker && git push

# auth-client (npm) — auth-worker の push 時に GitHub Actions で自動 publish

# nuxt-pwa-carins (Cloudflare Workers) — ビルド必須
cd /home/yhonda/js/nuxt-pwa-carins && npm run build && npx wrangler deploy

# nuxt-items (Cloudflare Workers) — ビルド必須
cd /home/yhonda/js/nuxt-items && npm run build && npx wrangler deploy

# cf-grpc-proxy (Cloudflare Workers)
cd /home/yhonda/js/nuxt_dtako_logs/cf-grpc-proxy && npx wrangler deploy

# smb-upload-worker (Cloudflare Workers)
cd /home/yhonda/js/smb-upload-worker && npx wrangler deploy
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
    - `packages/auth-client/` — 共通 Nuxt composable（`@yhonda-ohishi-pub-dev/auth-client`、GitHub Packages で公開）
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

## npmパッケージ (@yhonda-ohishi-pub-dev/auth-client)

フロントエンド共通の認証 composable（LINE WORKS 自動ログイン対応）。GitHub Packages で公開。

### ソース

`workers/auth-worker/packages/auth-client/` → `auth-worker` リポジトリ内

### GitHub Actions 自動公開

`auth-worker` リポジトリの `.github/workflows/publish-auth-client.yml` で `main` push 時に自動 publish。
バージョン: `0.1.{COMMIT_COUNT}`（コミット数ベース）

### 使用先

- `front/nuxt-pwa-carins/` — `composables/useAuth.ts` で re-export
- `front/nuxt-dtako-logs/` — `composables/useAuth.ts` で re-export

両プロジェクトで `nuxt.config.ts` の `build.transpile` に追加が必要（TypeScript ソース直接配布のため）。

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

## 完了: Rich Menu 画像アップロード 401 エラー修正

### 原因
`uploadImage()` Step 2 の実装が LINE WORKS API 仕様と3点で異なっていた:
- HTTP メソッド: `PUT` → 正しくは **`POST`**
- Content-Type: `image/png` (raw binary) → 正しくは **`multipart/form-data`**
- Body: raw ArrayBuffer → 正しくは **FormData** (`resourceName` + `Filedata` フィールド)

### 修正ファイル
- `auth-worker/src/lib/lineworks-bot-api.ts:242-257` — `uploadImage()` Step 2 を `FormData` + `POST` に変更

### LINE WORKS ファイルアップロード仕様（重要）
uploadUrl へのアップロードは以下の形式:
```
POST {uploadUrl}
Authorization: Bearer {token}
Content-Type: multipart/form-data
FormData: resourceName={fileName}, Filedata={binaryFile}
```
公式ドキュメント: https://developers.worksmobile.com/jp/reference/file-upload?lang=ja

---

## 完了: 複数組織対応 — 組織切り替え機能

### 背景
ユーザーが複数の組織に所属する場合、ログイン後に組織を切り替える手段がなかった。`user_organizations` テーブル（多対多）と `ListMyOrganizations` RPC は既存。不足していた「組織切り替え → 新JWT発行」フローとフロントエンドUIを追加。

### 方式: SwitchOrganization RPC（新JWT発行）
- JWT の `org` / `org_slug` が常に正しく反映される
- cookie による cross-subdomain 共有も正しい org を反映

### 変更ファイル

| リポジトリ | ファイル | 変更内容 |
|-----------|---------|---------|
| rust-logi | `packages/logi-proto/proto/auth.proto` | `SwitchOrganization` RPC + `SwitchOrganizationRequest` 追加 |
| rust-logi | `src/services/auth_service.rs` | `switch_organization` 実装（メンバーシップ確認 + JWT再発行） |
| auth-worker | `src/handlers/api-switch-org.ts` | POST `/api/switch-org`（CORS対応） |
| auth-worker | `src/handlers/api-my-orgs.ts` | POST `/api/my-orgs`（CORS対応） |
| auth-worker | `src/index.ts` | ルート + OPTIONS preflight 追加 |
| auth-worker | `src/lib/errors.ts` | `extractToken`, `corsJsonResponse`, `corsPreflight` 共通ユーティリティ |
| auth-client | `packages/auth-client/src/useAuth.ts` | `OrgInfo`, `fetchOrganizations`, `switchOrganization`, `isMultiOrg` 追加 |
| auth-client | `packages/auth-client/src/AuthToolbar.vue` | 複数組織時ドロップダウン切替UI |
| nuxt-pwa-carins | `plugins/auth.client.ts` | 認証後に `fetchOrganizations()` 呼び出し |
| nuxt-items | `plugins/auth.client.ts` | 同上 |

### auth_service.rs の switch_organization 実装ポイント
- `AuthenticatedUser` は `username` を持たないため、`app_users` テーブルから `COALESCE(email, display_name)` で取得
- `user_organizations` + `organizations` JOIN でメンバーシップ確認 + `org_slug` 取得
- PUBLIC_PATHS に追加しない（JWT認証必須）

### auth-worker の共通ユーティリティ（lib/errors.ts）
```typescript
extractToken(request)    // Authorization: Bearer <token> からトークン抽出
corsJsonResponse(data)   // JSON レスポンス + CORS ヘッダー
corsPreflight()          // OPTIONS preflight レスポンス
```
新しい cross-origin API ハンドラではこれらを使うこと。

### フロントエンドの動作
1. 認証成功後 `fetchOrganizations()` で所属組織一覧取得
2. 複数組織の場合、AuthToolbar に組織切替ドロップダウン表示
3. 選択時 `switchOrganization(orgId)` → 新JWT発行 → `window.location.reload()`

### デプロイ済み
- rust-logi: Cloud Run
- auth-worker: Cloudflare Workers (`auth.mtamaramu.com`)
- auth-client: GitHub Packages (`@yhonda-ohishi-pub-dev/auth-client`)
- nuxt-pwa-carins: Cloudflare Pages
- nuxt-items: Cloudflare Workers (`items.mtamaramu.com`)

---

## 完了: バーコード検索 — 楽天製品検索API統合

### 背景
items.mtamaramu.com のバーコード検索はローカルDB（gRPC `SearchByBarcode`）のみで、未登録商品の検索ができなかった。楽天製品検索APIを統合し、ローカル0件時に外部DBから商品情報を取得して表示 + ワンクリック登録できるようにした。

### 検索順序
1. **楽天製品検索API**（日本商品に強い） → サーバールート `/api/barcode-lookup` 経由
2. **OpenFoodFacts**（海外食品フォールバック）
3. **OpenProductsFacts**（海外非食品フォールバック）

### 変更ファイル

| リポジトリ | ファイル | 変更内容 |
|-----------|---------|---------|
| nuxt-items | `server/api/barcode-lookup.ts` | **新規**: 楽天APIプロキシ（Refererヘッダー必須のためサーバーサイド） |
| nuxt-items | `composables/useProductLookup.ts` | 楽天APIを最優先ソースとして追加、OpenFacts系をフォールバックに |
| nuxt-items | `pages/index.vue` | 検索フローに外部API統合（0件時自動検索 + 結果表示UI） |
| nuxt-items | `components/items/ItemForm.vue` | `initialBarcode` prop追加、バーコード欄をフォーム先頭に移動 |

### 楽天API設定
- **エンドポイント**: `https://openapi.rakuten.co.jp/ichibaproduct/api/Product/Search/20250801`
- **検索パラメータ**: `productCode`（JANコード直接指定）
- **認証**: `applicationId` + `accessKey`（`wrangler secret` で管理）
- **ローカル開発**: `nuxt-items/.env` にキー設定
- **注意**: Refererヘッダー必須のため、ブラウザ直接呼び出し不可（サーバールート経由必須）

### デプロイ済み
- nuxt-items: Cloudflare Workers (`items.mtamaramu.com`)

---

## 完了: items.mtamaramu.com マルチブラウザ同期

### 背景
items.mtamaramu.com はリクエスト-レスポンス型の gRPC-Web 通信のみで、複数ブラウザ/デバイス間でのリアルタイム同期機能がなかった。Cloudflare Durable Objects の WebSocket Hibernation API を使い、クロスデバイスのリアルタイム同期を実装。

### アーキテクチャ
```
Browser A ──WebSocket──┐
Browser B ──WebSocket──┤→ ItemsSyncDO (Hibernation API)
Browser C ──WebSocket──┘    Room: items-{orgId}

CRUD flow:
  Browser A: gRPC create → success → WS notify → ItemsSyncDO → broadcast
  Browser B: receive notification → refetch items
  同一ブラウザ他タブ: BroadcastChannel → refetch items
```

### 接続先
- WebSocket: `wss://sync.mtamaramu.com/ws/items/{orgId}?token=JWT`

### 変更ファイル

| リポジトリ | ファイル | 変更内容 |
|-----------|---------|---------|
| cf-grpc-proxy | `wrangler.toml` | `ItemsSyncDO` DO binding + migration + `sync.mtamaramu.com` カスタムドメイン |
| cf-grpc-proxy | `src/index.ts` | `ItemsSyncDO` クラス (Hibernation API) + fetch handler WS ルーティング |
| nuxt-items | `composables/useItemsSync.ts` | **新規**: WebSocket + BroadcastChannel 同期 composable |
| nuxt-items | `composables/useItems.ts` | CRUD 後に `sync.notifyChange()` + `initSync()` メソッド追加 |
| nuxt-items | `pages/index.vue` | `initSync()` 呼び出し + 同期状態ドット表示 |
| nuxt-items | `wrangler.toml` | `NUXT_PUBLIC_SYNC_URL` 環境変数追加 |
| nuxt-items | `nuxt.config.ts` | `runtimeConfig.public.syncUrl` 追加 |

### ItemsSyncDO 設計ポイント
- **Hibernation API**: アイドル時の課金ゼロ、`setWebSocketAutoResponse` で ping/pong を DO 起動なしで自動応答
- **Room 粒度**: org_id 単位（`items-{orgId}`）— 1 DO インスタンスに同一組織の全クライアント
- **personal items フィルタ**: `serializeAttachment({ userId })` で送信元を記録、personal アイテム変更は同一ユーザーの別デバイスにのみ通知
- **メッセージ**: 軽量通知のみ（type, action, parentId, ownerType）→ クライアントが refetch
- **JWT 認証**: WebSocket 接続時にクエリパラメータの JWT を既存の `verifyJwt()` で検証

### クライアント同期ロジック
- WebSocket: クロスデバイス同期（exponential backoff + jitter で自動再接続、visibilitychange で非表示タブの再接続抑制）
- BroadcastChannel: 同一ブラウザのタブ間即時同期（サーバー不要）
- 受信時: 同じフォルダ + ownerType 表示中なら即座に refetch、別フォルダは無視（ナビゲーション時に常に fetch するため）

### デプロイ済み
- cf-grpc-proxy: Cloudflare Workers (`sync.mtamaramu.com`)
- nuxt-items: Cloudflare Workers (`items.mtamaramu.com`)

---

## 完了: LINE WORKS 自動ログイン + useAuth 共通化

### 背景
LINE WORKS アプリ内からフロントエンドを開いた際、ログインページで手動で LINE WORKS アドレスを入力する必要があった。Bot リンクに `?lw=<domain>` パラメータを付与することでログインページをスキップし、LINE WORKS OAuth を自動開始する機能を追加。

### 共通パッケージ: `@yhonda-ohishi-pub-dev/auth-client`
- **リポジトリ**: `auth-worker/packages/auth-client/`
- **公開先**: GitHub Packages（GitHub Actions で自動 publish）
- **内容**: `useAuth` composable（JWT 管理 + LINE WORKS 自動ログイン）
- 両フロントエンドの `composables/useAuth.ts` を共通化

### フロー
```
Bot リンク: https://carins.mtamaramu.com/?lw=ohishi
  → サーバーミドルウェア or クライアントプラグインが ?lw=ohishi を検出
  → lw_domain を localStorage/cookie に保存
  → auth-worker /oauth/lineworks/redirect に直接リダイレクト（ログインページスキップ）
  → LINE WORKS OAuth（アプリ内なので自動承認）
  → JWT 取得 → アプリ表示
次回以降: 保存済み lw_domain で自動ログイン（?lw パラメータ不要）
```

### 変更ファイル

**auth-worker** (commit: `176d713`):
- `packages/auth-client/package.json` — 新規パッケージ定義
- `packages/auth-client/src/useAuth.ts` — 共通 composable（LW 自動ログイン付き）
- `packages/auth-client/src/index.ts` — re-export
- `.github/workflows/publish-auth-client.yml` — GitHub Packages publish

**nuxt-pwa-carins**:
- `composables/useAuth.ts` → `@yhonda-ohishi-pub-dev/auth-client` の re-export に置換
- `plugins/auth.client.ts` — `?lw=<domain>` 処理 + cookie 同期追加
- `server/middleware/auth.ts` — `?lw` / `lw_domain` cookie / `?lw_callback` 対応
- `nuxt.config.ts` — `transpile` に auth-client 追加

**nuxt-dtako-logs**:
- `composables/useAuth.ts` → `@yhonda-ohishi-pub-dev/auth-client` の re-export に置換
- `plugins/auth.client.ts` — `?lw=<domain>` 処理 + cookie 同期追加
- `nuxt.config.ts` — `transpile` に auth-client 追加

### 注意事項
- `?lw=<domain>` は初回のみ必要（Bot がドメイン付き URL を送信）、以降は localStorage/cookie で記憶
- 明示的ログアウト時は `clearLwDomain()` でドメイン記憶を解除
- auth-worker の既存エンドポイントに変更なし（`/oauth/lineworks/redirect?address=<domain>` がそのまま動作）
- SSR ミドルウェアのリダイレクトループ防止に `?lw_callback=1` マーカーを使用

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
