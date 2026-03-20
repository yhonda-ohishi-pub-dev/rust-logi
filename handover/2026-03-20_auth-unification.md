# Auth統合 — auth-worker → rust-alc-api 移行

## 日付: 2026-03-20

## 完了したこと

### Step 1: rust-alc-api に認証エンドポイント実装
- `GET /api/auth/woff-config` — WOFF SDK 設定取得
- `POST /api/auth/woff` — WOFF SDK 認証
- `GET /api/auth/lineworks/redirect` — LINE WORKS OAuth 開始（`address` パラメータ対応）
- `GET /api/auth/lineworks/callback` — LINE WORKS OAuth コールバック
- `GET /api/auth/google/redirect` — Google OAuth 開始
- `GET /api/auth/google/callback` — Google OAuth コールバック
- カスタムドメイン `alc-api.ippoan.org` 設定済み（Cloud Run）

### Step 3: JWT 統一
- JWT に `tenant_id` + `org_slug` を含めるように修正
- `create_access_token()` に `org_slug` パラメータ追加
- 全ての JWT 発行箇所（Google/LINE WORKS/WOFF/refresh）で tenant の slug を取得して設定

### Step 4: フロントエンド認証先変更（nuxt-pwa-carins のみ完了）
- `NUXT_PUBLIC_AUTH_WORKER_URL` を `https://alc-api.ippoan.org` に変更
- auth-client の `consumeFragment` で `tenant_id` / `org` 両方対応
- auth-client の `decodeJwtClaims` で `email` をフォールバックに追加
- SSR ミドルウェアのデフォルトリダイレクトを `auth.mtamaramu.com/login` に設定
- logout 時に `auth.mtamaramu.com/login` にリダイレクト
- `AuthToolbar` に `show-org-slug` prop 追加
- `AuthToolbar` に `Apps` ボタン追加（`auth.ippoan.org/top`）

### auth-worker の変更
- ログインページの Google/LINE WORKS ボタンを rust-alc-api にルーティング
- `ALC_API_ORIGIN` 環境変数追加
- `auth.ippoan.org` カスタムドメイン追加
- admin/sso API を gRPC → rust-alc-api REST に切り替え
- bot config API を gRPC → rust-alc-api REST に切り替え
- admin/sso ページのリダイレクトをリクエスト origin ベースに変更

### rust-alc-api 管理 API
- `GET/POST/DELETE /api/admin/sso/configs` — SSO 設定 CRUD
- `GET/POST/DELETE /api/admin/bot/configs` — Bot 設定 CRUD
- `bot_configs` テーブル作成（migration 049）
- RLS 対応（`set_current_tenant` 呼び出し）

### DB 修正
- `sso_provider_configs.tenant_id` を `536859de-...` に更新
- `users` テーブルの LINE WORKS ユーザーの `tenant_id` を `536859de-...` に更新
- `tenants` テーブルの slug を `ohishiunyusouko` に設定
- `bot_configs` データを logi スキーマから alc_api スキーマに移行

### auth-client バージョン: 0.1.30
- GitHub Packages: `@yhonda-ohishi-pub-dev/auth-client@0.1.30`

## 未完了

### nuxt-items の rust-logi → rust-alc-api 移行
- items.mtamaramu.com は cf-grpc-proxy → rust-logi の gRPC を使用
- rust-alc-api の JWT（`tenant_id`）を rust-logi が認証できない（`org` を期待）
- **やること:**
  1. nuxt-items が使う gRPC エンドポイントを洗い出す
  2. rust-alc-api に対応する REST API があるか確認（items 関連は migration 048 で移行済みの可能性）
  3. 不足している REST API を追加
  4. nuxt-items の composables を gRPC → REST に切り替え
  5. wrangler.toml の `cf-grpc-proxy` binding を削除

### nuxt-dtako-logs の移行
- ohishi2.mtamaramu.com も同様に cf-grpc-proxy → rust-logi
- こちらは dtakologs 固有のエンドポイント

### auth-worker 完全廃止（Step 5-6）
- auth.mtamaramu.com のログインページはまだ使用中（logout 時のリダイレクト先）
- `/top` アプリ一覧ページもまだ auth-worker で提供
- rust-alc-api にログインページ + アプリ一覧を実装すれば廃止可能

### auth-client publish の自動化問題
- GitHub Actions のバージョン計算がコミット数ベースで、同じバージョンが publish される場合がある
- 手動で `npm version` + `npm publish` が必要な場合あり

## 注意事項
- `alc-api.ippoan.org` は Cloud Run のカスタムドメイン（DNS は Cloudflare, proxy OFF）
- `auth.ippoan.org` は auth-worker のカスタムドメイン（wrangler custom_domain）
- SSO 暗号化キーは `JWT_SECRET` のSHA-256ハッシュ（rust-logi と同じ方式）
- `sso_provider_configs` と `bot_configs` は FORCE RLS — クエリ前に `set_current_tenant` 必須
- migration 049 の `bot_configs` テーブルオーナーは `alc_api_app` に変更済み（手動 ALTER）
