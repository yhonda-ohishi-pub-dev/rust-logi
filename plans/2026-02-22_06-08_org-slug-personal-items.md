# org_slug JWT 追加 + 個人物品管理設計

## Context

2つの課題:
1. SSO 管理ページで `/join/<org_slug>` URL を表示するため、JWT claims に `org_slug` を追加（案 D）
2. 物品管理を組織と個人の両方で行うための DB 設計（アプローチ B: 単一テーブル dual ownership）

---

## Phase 1: JWT claims に org_slug 追加（案 D）

### rust-logi

- [x] **1.1** `src/services/auth_service.rs` — Claims に `org_slug: String` (`#[serde(default)]`) 追加
- [x] **1.2** `src/services/auth_service.rs` — `issue_jwt()` に `org_slug` 引数追加
- [x] **1.3** `src/services/auth_service.rs` — `sign_up_with_google` で slug を渡す（作成時に既知）
- [x] **1.4** `src/services/auth_service.rs` — `login_with_google` の既存ユーザー検索 SQL に `JOIN organizations o ON o.id = uo.organization_id` + `o.slug` 追加
- [x] **1.5** `src/services/auth_service.rs` — `login_with_google` の auto-register（デフォルト org）で slug を取得して渡す
- [x] **1.6** `src/services/auth_service.rs` — `login_with_sso_provider` で org slug を JOIN で取得
- [x] **1.7** `src/services/auth_service.rs` — `login` (password) で org slug を JOIN で取得
- [x] **1.8** `src/middleware/auth.rs` — Claims deserialization に `org_slug` 追加、AuthenticatedUser に `org_slug` フィールド追加
- [x] **1.9** `cargo build --release` 成功確認
- [x] **1.9b** `src/services/member_service.rs` — `accept_invitation` の `issue_jwt()` + invitation query にも org_slug 追加

### auth-worker

- [x] **1.10** `src/lib/admin-html.ts` — `initAuth()` で JWT decode → `payload.org_slug` → join URL 構築
- [x] **1.11** `wrangler deploy --dry-run` ビルド確認

### デプロイ・検証

- [x] **1.12** rust-logi デプロイ — revision `rust-logi-00107-w82` (SERVING)
- [x] **1.13** auth-worker デプロイ — version `d218c396-2774-4139-8707-98dea53233fe`
- [ ] **1.14** ログイン → JWT decode → `org_slug` フィールド確認
- [ ] **1.15** SSO admin ページで join URL が正しく表示されることを確認

---

## Phase 2: 個人物品管理（実装中）

**決定済み**: アプローチ B（単一テーブル + dual ownership）

### DB マイグレーション

- [x] **2.1** user_id ベース RLS 関数作成 — `migrations/00029_create_items.sql`
- [x] **2.2** items テーブル作成（dual ownership + 階層 parent_id + barcode + quantity）
- [x] **2.3** `sqlx migrate run` でマイグレーション適用

### Rust 実装

- [x] **2.4** `src/db/organization.rs` に `set_current_user()` ヘルパー追加
- [x] **2.5** Proto `packages/logi-proto/proto/items.proto` 作成（7 RPC: CreateItem, GetItem, UpdateItem, DeleteItem, ListItems, MoveItem, SearchByBarcode）
- [x] **2.6** `src/services/items_service.rs` 実装
  - dual RLS: `setup_dual_rls()` で `set_current_organization()` + `set_current_user()` を同一コネクションで呼出し
  - PERMISSIVE ポリシーで org OR personal の行が自動的に見える
- [x] **2.7** `cargo build --release` 成功確認
- [x] **2.7b** 登録ファイル修正（build.rs, proto/mod.rs, models/mod.rs, services/mod.rs, main.rs, index.ts）

### デプロイ・検証

- [x] **2.8** Cloud Run にデプロイ — revision `rust-logi-00108-n6g` (SERVING)
- [x] **2.9** gRPC reflection で ItemsService 確認（7 RPC 全て表示）
- [x] **2.10** CreateItem → ListItems → DeleteItem の CRUD フロー確認済み

### フロントエンド（将来）

- [ ] **2.11** `front/nuxt-items/` に symlink 方式で Nuxt プロジェクト配置
- [ ] **2.12** 物品管理 UI（組織/個人切替、階層表示、バーコード検索）
- [ ] **2.13** QR 生成 + ラベルプリンター印刷

---

## 設計メモ

### dual ownership RLS の仕組み

PostgreSQL の PERMISSIVE ポリシーは OR で結合される:
- `set_current_organization()` 呼出し → `items_org_rls` が組織データを許可
- `set_current_user()` 呼出し → `items_personal_rls` が個人データを許可
- 両方未設定 → 0件（FORCE RLS により安全）

### gRPC ハンドラルール

| データ種別 | 呼ぶ関数 | ソース |
|-----------|---------|--------|
| 組織データ | `set_current_organization(&mut conn, &org_id)` | `get_organization_from_request()` |
| 個人データ | `set_current_user(&mut conn, &user_id)` | `AuthenticatedUser.user_id` |

### 移管（個人→組織）

```sql
UPDATE items
SET owner_type = 'org', organization_id = $1, user_id = NULL
WHERE id = $2 AND owner_type = 'personal' AND user_id = $3;
```

### 重要ファイル

| ファイル | 役割 |
|---------|------|
| `src/services/auth_service.rs` | JWT 発行（Claims + issue_jwt） |
| `src/middleware/auth.rs` | JWT 検証（AuthenticatedUser） |
| `src/db/organization.rs` | RLS ヘルパー（set_current_organization, set_current_user） |
| `migrations/00029_create_items.sql` | items テーブル + RLS ポリシー |
| `packages/logi-proto/proto/items.proto` | 物品管理 Proto（7 RPC） |
| `src/models/item.rs` | ItemModel（FromRow） |
| `src/services/items_service.rs` | 物品管理サービス（dual RLS） |
