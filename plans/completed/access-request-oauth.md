# OAuth アカウント招待・承認管理機能

## Context

公開URL `/join/<org-slug>` で任意の OAuth プロバイダー（Google, LINE WORKS, 将来追加分）を使って組織への参加をリクエストし、管理者が LINE WORKS 通知 + 管理画面から承認/却下する機能。

## 設計方針

- **プロバイダー非依存**: `CreateAccessRequest` RPC は認証済み JWT からユーザー情報を取得。Google / LINE WORKS / 将来のプロバイダーすべて共通
- **既存 OAuth フロー再利用**: join ページから既存の Google / LINE WORKS OAuth redirect に `join_org` パラメータを追加するだけ。callback で `join_org` を検出したら参加リクエストフローに分岐
- **RPC は認証済み**: OAuth 完了 → JWT 取得 → `CreateAccessRequest(org_slug)` を JWT 付きで呼び出し

## ユーザーフロー

```
1. /join/<org-slug> にアクセス → 組織名 + OAuth ボタン表示
2. Google or LINE WORKS ボタンをクリック
3. 既存 OAuth フロー（join_org パラメータ付き）
4. OAuth callback で join_org を検出 → join-callback に転送
5. LoginWithGoogle / LoginWithSsoProvider で JWT 取得
6. CreateAccessRequest(org_slug) を JWT 付きで呼び出し
7. access_request レコード作成 + LINE WORKS Bot 通知
8. 結果ページ表示
9. 管理者が /admin/requests で承認/却下
```

---

## チェックリスト

### Phase 1: rust-logi バックエンド

- [x] **1.1** マイグレーション `migrations/00028_create_access_requests.sql` 作成
  - access_requests テーブル（provider カラム付き）
  - partial unique index、RLS（NOT FORCE）
- [x] **1.2** マイグレーション実行 `sqlx migrate run` (00027 + 00028 適用済み)
- [x] **1.3** Proto `packages/logi-proto/proto/access_request.proto` 作成
  - `CreateAccessRequest(org_slug)` — 認証必須、JWT から user_id 取得
  - `GetOrganizationBySlug(slug)` — 認証不要
  - `ListAccessRequests(status_filter)` — admin のみ
  - `ApproveAccessRequest(request_id, role)` — admin のみ
  - `DeclineAccessRequest(request_id)` — admin のみ
- [x] **1.4** `build.rs` に `access_request.proto` 追加
- [x] **1.5** `src/services/access_request_service.rs` 実装
  - AuthenticatedUser に `provider` フィールド追加 (middleware/auth.rs)
- [x] **1.6** `src/services/mod.rs` にモジュール追加
- [x] **1.7** `src/main.rs` にサービス追加
- [x] **1.8** `src/middleware/auth.rs` の PUBLIC_PATHS に `GetOrganizationBySlug` 追加
  - `CreateAccessRequest` は認証必須なので PUBLIC_PATHS に追加しない
- [x] **1.9** `cargo build --release` 成功確認

### Phase 2: logi-proto npm パッケージ

- [x] **2.1** TypeScript 生成 + `index.ts` に export 追加 + auth-worker に npm install

### Phase 3: auth-worker フロントエンド

- [x] **3.1** `src/lib/security.ts` — 戻り値型に `join_org?: string` 追加
- [x] **3.2** `src/handlers/google-redirect.ts` — `join_org` パラメータ対応
  - `src/handlers/lineworks-redirect.ts` も同様に対応
- [x] **3.3** `src/handlers/join-page.ts` 新規 — join ページハンドラー
- [x] **3.4** `src/lib/join-html.ts` 新規 — join ページ + 結果ページ + 404 ページ HTML
- [x] **3.5** `src/handlers/join-callback.ts` 新規 — /join/:slug/done (クライアントサイド JS で API 呼び出し)
  - `src/handlers/google-callback.ts` に join 分岐追加
  - `src/handlers/lineworks-callback.ts` に join 分岐追加
- [x] **3.6** `src/handlers/admin-requests.ts` 新規 — admin 管理ページ + callback
- [x] **3.7** `src/lib/admin-requests-html.ts` 新規 — admin 管理ページ HTML (SPA)
- [x] **3.8** `src/handlers/api-access-requests.ts` 新規 — create/list/approve/decline API
- [x] **3.9** `src/index.ts` — 全ルーティング追加 (/join/:slug, /admin/requests, /api/access-requests/*)
  - `wrangler deploy --dry-run` ビルド成功確認

### Phase 4: デプロイ・検証

- [x] **4.1** Google Cloud Console に redirect URI 追加（不要 — 既存の callback URL を再利用）
- [x] **4.2** rust-logi デプロイ (`./deploy.sh`) — revision rust-logi-00106-zjv
- [x] **4.3** auth-worker デプロイ (`wrangler deploy`) — version 5faed5b9
- [x] **4.4** E2E テスト — 全エンドポイント動作確認済み

---

## 詳細設計

### DB マイグレーション

```sql
CREATE TABLE access_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    user_id UUID NOT NULL REFERENCES app_users(id),
    email TEXT NOT NULL,
    display_name TEXT NOT NULL DEFAULT '',
    avatar_url TEXT,
    provider TEXT NOT NULL DEFAULT '',  -- 'google', 'lineworks', etc.
    status TEXT NOT NULL DEFAULT 'pending',
    role TEXT,
    reviewed_by UUID REFERENCES app_users(id),
    reviewed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

### Proto 定義

```protobuf
service AccessRequestService {
  rpc CreateAccessRequest(CreateAccessRequestReq) returns (CreateAccessRequestRes); // 認証必須
  rpc GetOrganizationBySlug(GetOrgBySlugReq) returns (GetOrgBySlugRes);            // 認証不要
  rpc ListAccessRequests(ListAccessRequestsReq) returns (ListAccessRequestsRes);    // admin
  rpc ApproveAccessRequest(ApproveAccessRequestReq) returns (Empty);                // admin
  rpc DeclineAccessRequest(DeclineAccessRequestReq) returns (Empty);                // admin
}

message CreateAccessRequestReq {
  string org_slug = 1;  // user_id, email, provider は JWT から取得
}
```

### auth-worker join フロー

join ページは既存 OAuth redirect を再利用:
- Google: `/oauth/google/redirect?redirect_uri={AUTH_WORKER_ORIGIN}/join/{slug}/done&join_org={slug}`
- LINE WORKS: `/oauth/lineworks/redirect?address={domain}&redirect_uri={AUTH_WORKER_ORIGIN}/join/{slug}/done&join_org={slug}`

callback で `join_org` を検出 → JWT 取得後、join-callback に転送して `CreateAccessRequest` RPC 呼び出し。

### 重要ファイル

| ファイル | アクション |
|---------|-----------|
| `rust-logi/migrations/00028_create_access_requests.sql` | 新規 |
| `rust-logi/packages/logi-proto/proto/access_request.proto` | 新規 |
| `rust-logi/build.rs` | 修正 |
| `rust-logi/src/services/access_request_service.rs` | 新規 |
| `rust-logi/src/services/mod.rs` | 修正 |
| `rust-logi/src/main.rs` | 修正 |
| `rust-logi/src/middleware/auth.rs` | 修正 |
| `auth-worker/src/index.ts` | 修正 |
| `auth-worker/src/lib/security.ts` | 修正 |
| `auth-worker/src/handlers/google-redirect.ts` | 修正 |
| `auth-worker/src/handlers/join-page.ts` | 新規 |
| `auth-worker/src/handlers/join-callback.ts` | 新規 |
| `auth-worker/src/handlers/admin-requests.ts` | 新規 |
| `auth-worker/src/handlers/api-access-requests.ts` | 新規 |
| `auth-worker/src/lib/join-html.ts` | 新規 |
| `auth-worker/src/lib/admin-requests-html.ts` | 新規 |

---

## 未解決: SSO 管理ページで join URL をコピーする際の org_slug 取得方法

### 現状

SSO 管理ページ (`/admin/sso`) に参加リクエスト URL のコピーボタンを追加したい。
join URL は `https://auth.mtamaramu.com/join/<org_slug>` だが、クライアント側に `org_slug` がない。
暫定で `externalOrgId`（LINE WORKS ドメイン）を使っているが、概念的に別物。

### 案 A: `ListConfigsRes` に `org_slug` を追加（SSO サービス）

**Pros**
- SSO ページは既に `listConfigs()` を呼んでいるので追加 API 不要
- SSO 設定と一緒に org 情報が返るのは自然

**Cons**
- SSO 設定と org slug は関心が異なる（SsoSettingsService に org 情報を混ぜる）
- proto + Rust + npm rebuild + auth-worker 変更が必要

### 案 B: 新 RPC `GetMyOrganization` を AccessRequestService に追加

**Pros**
- 関心の分離が明確（org 情報は AccessRequestService が担当）
- 将来、org 設定画面など他の用途にも使える

**Cons**
- proto + Rust + npm rebuild + auth-worker API 追加 + admin ページで追加 fetch
- SSO ページのロード時に API が 2 本になる（listConfigs + getMyOrganization）

### 案 C: `handleAdminSsoPage` でサーバーサイドで slug を取得し HTML に埋め込む

**Pros**
- クライアント側の追加 fetch 不要（HTML テンプレートに slug を埋める）
- admin-sso.ts のみ変更、admin-html.ts は引数追加のみ

**Cons**
- auth-worker から slug を取得する RPC が必要（案 B と同じ proto 変更）
- SSO ハンドラが AccessRequestService にも依存する

### 案 D: JWT claims に `org_slug` を追加

**Pros**
- クライアント側で `atob(token.split('.')[1])` で即座に取得可能
- API 追加不要、admin ページの `initAuth()` に数行追加のみ

**Cons**
- JWT payload サイズが増える（全リクエストに影響）
- slug 変更時に既存 JWT が古い値を持つ
- Rust `Claims` struct + JWT 生成ロジック変更が必要

### 案 E: `externalOrgId` = `org_slug` を前提とする（現状）

**Pros**
- 変更不要、既に動作する

**Cons**
- 概念的に別物（LINE WORKS ドメイン vs org slug）
- SSO 未設定の組織では join URL が表示されない
- 将来 slug を変更した場合に壊れる
