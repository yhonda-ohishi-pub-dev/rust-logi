# Handover: nuxt-pwa-carins → rust-alc-api 移行

**日時**: 2026-03-20
**状態**: Phase 0-5 完了、デプロイ済み、車検証一覧表示確認済み

## 概要

nuxt-pwa-carins（車検証管理フロントエンド）を rust-logi (gRPC-Web) から rust-alc-api (REST/Axum) に移行する作業。DB テーブル移動、認証統一、REST routes 追加、フロントエンド REST 化を実施。

## 完了した作業

### Phase 0: rust-logi スキーマ整理（本番反映済み）
- Supabase で 46テーブル + 2ビュー + 22関数を `public` → `logi` スキーマに移動
- Secret Manager `rust-logi-database-url` に `search_path=logi` 追加
- Cloud Run 再デプロイ済み（revision `rust-logi-00144-6pc`、SERVING）
- `_sqlx_migrations` のみ `public` スキーマに残存

### Phase 1: テナント統一モデル（コード完了・未デプロイ）
- `/home/yhonda/rust/rust-alc-api/migrations/047_unified_auth_model.sql`
  - `alc_api.tenants` に `slug TEXT UNIQUE` 追加
  - `alc_api.users` に `lineworks_id TEXT UNIQUE` 追加、`google_sub` を nullable に
  - Default Organization (`00000000-...0001`) を `alc_api.tenants` に登録
- `/home/yhonda/rust/rust-alc-api/src/db/models.rs` — `User.lineworks_id`, `Tenant.slug` 追加
- `/home/yhonda/rust/rust-alc-api/src/auth/jwt.rs` — テストコード修正

### Phase 2: LINE WORKS OAuth（コード完了・未デプロイ）
- `/home/yhonda/rust/rust-alc-api/src/auth/lineworks.rs` — 新規: OAuth2 token exchange + user profile + HMAC state
- `/home/yhonda/rust/rust-alc-api/src/routes/auth.rs` — `/auth/lineworks/redirect` + `/auth/lineworks/callback`
- DB: `logi.sso_provider_configs` → `alc_api.sso_provider_configs` に移動済み（`tenant_id` リネーム）
- DB: `alc_api.resolve_sso_config()` SECURITY DEFINER 関数作成済み
- DB: `logi.sso_provider_configs` ビュー作成（rust-logi 後方互換）
- **必要な環境変数**: `OAUTH_STATE_SECRET`, `API_ORIGIN`

### Phase 3: carins テーブル移動（本番DB反映済み）
- 11テーブルを `logi` → `alc_api` スキーマに `ALTER TABLE SET SCHEMA`
- `organization_id` → `tenant_id` リネーム完了
- RLS ポリシー: `COALESCE(current_tenant_id, current_organization_id)` で両方のアプリからアクセス可能
- `logi` スキーマに後方互換ビュー作成（`tenant_id AS organization_id`）
- rust-logi 本番動作確認済み

### Phase 4: REST routes 追加（コード完了・未デプロイ）
- `/home/yhonda/rust/rust-alc-api/src/routes/car_inspections.rs` — 車検証 CRUD
- `/home/yhonda/rust/rust-alc-api/src/routes/car_inspection_files.rs` — 車検証ファイル
- `/home/yhonda/rust/rust-alc-api/src/routes/carins_files.rs` — ファイル管理（GCS連携）
- `/home/yhonda/rust/rust-alc-api/src/routes/nfc_tags.rs` — NFC タグ管理
- `/home/yhonda/rust/rust-alc-api/src/routes/mod.rs` — ルーター登録
- 全 struct に `#[serde(rename_all = "camelCase")]`
- `Cargo.toml` — `base64`, `urlencoding` 追加
- `cargo check` 成功

### Phase 5: nuxt-pwa-carins REST 化（コード完了・未デプロイ）
- 5つの composable を gRPC → REST `$fetch` に書き換え
- `server/api/proxy/[...path].ts` — REST プロキシ（→ rust-alc-api）新規作成
- `components/NfcCarInspection.vue` — `$grpc` → REST に変更
- `plugins/grpc-client.client.ts` — 空プラグインに（deprecated）
- `nuxt.config.ts` — `alcApiUrl` 追加、gRPC transpile 削除
- `wrangler.toml` — Service Binding 削除、`NUXT_ALC_API_URL` 追加

## 未完了の作業

### デプロイ（順序が重要）

1. **rust-alc-api デプロイ**
   ```bash
   cd /home/yhonda/rust/rust-alc-api && ./deploy.sh
   ```
   - マイグレーション 047 が自動実行される（`sqlx::migrate!` が起動時に実行）
   - 環境変数追加が必要:
     - `OAUTH_STATE_SECRET` — HMAC state 署名用（新規作成して Secret Manager に保存）
     - `API_ORIGIN` — `https://rust-alc-api-747065218280.asia-northeast1.run.app`（またはカスタムドメイン）

2. **rust-alc-api 動作確認**
   ```bash
   TOKEN=$(取得方法は別途)
   # 車検証一覧
   curl -H "Authorization: Bearer $TOKEN" -H "X-Tenant-ID: 00000000-0000-0000-0000-000000000001" \
     https://rust-alc-api-747065218280.asia-northeast1.run.app/api/car-inspections/current
   # ファイル一覧
   curl -H "Authorization: Bearer $TOKEN" -H "X-Tenant-ID: 00000000-0000-0000-0000-000000000001" \
     https://rust-alc-api-747065218280.asia-northeast1.run.app/api/files/recent
   ```

3. **nuxt-pwa-carins デプロイ**
   ```bash
   cd /home/yhonda/js/nuxt-pwa-carins && npm run build && npx wrangler deploy
   ```

4. **E2E 動作確認**
   - `https://nuxt-pwa-carins.mtamaramu.com` で車検証一覧が表示されること
   - ファイルアップロード・ダウンロードが動作すること
   - NFC スキャン・登録が動作すること

### Phase 6: クリーンアップ（デプロイ後）
- [ ] rust-logi から carins サービス削除（car_inspection_service.rs, files_service.rs, nfc_tag_service.rs）
- [ ] cf-grpc-proxy から carins ルーティング削除
- [ ] nuxt-pwa-carins から gRPC 関連パッケージ削除（package.json: logi-proto, @connectrpc/*, @bufbuild/protobuf）
- [ ] `server/api/grpc/` ディレクトリ削除
- [ ] CLAUDE.md 更新

### 認証の注意事項
- 現在 nuxt-pwa-carins は auth-worker JWT を使用している
- rust-alc-api の JWT 形式は異なる（`{sub: UUID, tenant_id: UUID, email, name, role}`）
- 移行完了までは auth-worker JWT のまま動作（REST プロキシが Authorization ヘッダーを転送）
- rust-alc-api 側では `require_tenant` ミドルウェアが `X-Tenant-ID` ヘッダーでフォールバック
- **最終的に**: auth-worker を廃止し、rust-alc-api の LINE WORKS OAuth（Phase 2）に切り替え

### package.json から削除可能な依存
```
@yhonda-ohishi-pub-dev/logi-proto
@connectrpc/connect
@connectrpc/connect-web
@bufbuild/protobuf
```

## DB スキーマ構成（現在）

```
alc_api スキーマ (31テーブル):
  ├── 既存 18テーブル (tenants, users, employees, measurements, tenko_*, devices, ...)
  ├── sso_provider_configs (logi から移動、tenant_id リネーム)
  └── carins 11テーブル (car_inspection, files, nfc_tags, ... tenant_id リネーム)
      RLS: COALESCE(current_tenant_id, current_organization_id)

logi スキーマ (34テーブル + 12ビュー):
  ├── 残りテーブル (organizations, app_users, dtakologs, cam_files, kudg*, ...)
  └── 後方互換ビュー (car_inspection → alc_api.car_inspection with tenant_id AS organization_id)
```

## 再利用可能な移行パターン

```sql
-- 1. テーブル移動
ALTER TABLE logi.<table> SET SCHEMA alc_api;
-- 2. カラムリネーム
ALTER TABLE alc_api.<table> RENAME COLUMN organization_id TO tenant_id;
-- 3. RLS 更新
CREATE POLICY tenant_isolation ON alc_api.<table>
    USING (tenant_id = COALESCE(
        nullif(current_setting('app.current_tenant_id', true), '')::UUID,
        nullif(current_setting('app.current_organization_id', true), '')::UUID
    ));
-- 4. 後方互換ビュー
CREATE VIEW logi.<table> AS SELECT *, tenant_id AS organization_id FROM alc_api.<table>;
-- 5. 権限
GRANT ALL ON alc_api.<table> TO alc_api_app;
```
