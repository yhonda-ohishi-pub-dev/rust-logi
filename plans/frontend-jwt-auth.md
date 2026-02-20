# フロントエンドJWT認証の実装計画

## Context

rust-logiに組織管理・認証システム（Auth/Organization/Member）を追加済み。
バックエンドのJWT認証ミドルウェアは動作するが、フロントエンド（nuxt-pwa-carins）にログインUIがなく、JWTを送っていない。
現在は後方互換モードで動作中（JWTなしでもDEFAULT_ORGANIZATION_IDにフォールバック）。

## 問題: Authorizationヘッダーの衝突

```
Browser → Nuxt catch-all → Service Binding → cf-grpc-proxy → Cloud Run
                                                ↓
                                     Authorization: Bearer <GCP IAM token>
                                     （フロントのAuthorizationを上書き）
```

**解決策**: アプリJWTは `x-auth-token` ヘッダーで送る。

## 現在のアーキテクチャ

```
Browser (Connect RPC)
  ↓ POST /api/grpc/{service}/{method}
Nuxt catch-all (server/api/grpc/[...path].ts)
  ↓ Service Binding (Content-Type + Connect-Protocol-Version のみ転送)
cf-grpc-proxy (Cloudflare Worker + Durable Object)
  ↓ 全ヘッダーコピー + Authorization をGCP IAMトークンで上書き
Cloud Run (rust-logi)
  ↓ Auth Middleware → サービス
```

### 変更対象ファイル

| レイヤー | ファイル | 変更内容 |
|----------|----------|----------|
| rust-logi | `src/middleware/auth.rs` | `x-auth-token` からJWT読み取り |
| cf-grpc-proxy | `src/index.ts` | CORS に `X-Auth-Token` 追加 |
| nuxt-pwa-carins | `server/api/grpc/[...path].ts` | `x-auth-token` ヘッダー転送 |
| nuxt-pwa-carins | `plugins/grpc-client.client.ts` | interceptorで `x-auth-token` 付与 |
| nuxt-pwa-carins | 新規: `pages/login.vue` | ログイン画面 |
| nuxt-pwa-carins | 新規: `composables/useAuth.ts` | JWT管理（cookie保存） |
| nuxt-pwa-carins | 新規: `middleware/auth.ts` | Nuxtルートガード |

---

## Phase 1: バックエンド（rust-logi）

### 1-1. middleware/auth.rs — `x-auth-token` からJWT読み取り

```rust
// 変更: Authorization → x-auth-token
let auth_header = req
    .headers()
    .get("x-auth-token")
    .and_then(|v| v.to_str().ok())
    .map(|s| s.to_string());
```

- `src/middleware/auth.rs` L124-130
- `Bearer ` プレフィックスの strip は不要（直接トークン値を送る）
- JWTが無い場合は既存通りパススルー（後方互換維持）

### 1-2. デプロイ

```bash
cargo build && ./deploy.sh
```

---

## Phase 2: cf-grpc-proxy

### 2-1. CORS に X-Auth-Token 追加

- `cf-grpc-proxy_ref/src/index.ts` L25

```typescript
'Access-Control-Allow-Headers': '..., X-Auth-Token',
```

### 2-2. デプロイ

```bash
cd cf-grpc-proxy_ref && npx wrangler deploy
```

---

## Phase 3: フロントエンド（nuxt-pwa-carins）

### 3-1. catch-all proxy — x-auth-token 転送

- `server/api/grpc/[...path].ts` L23-35

```typescript
const headers = new Headers()
if (contentType) headers.set('Content-Type', contentType)
if (connectProtocol) headers.set('Connect-Protocol-Version', connectProtocol)
// 追加: x-auth-token を転送
const authToken = getHeader(event, 'x-auth-token')
if (authToken) headers.set('x-auth-token', authToken)
```

### 3-2. useAuth composable 新規作成

- `composables/useAuth.ts`

```typescript
// JWT管理
// - login(org_id, username, password) → AuthService/Login RPC
// - loginWithGoogle(id_token) → AuthService/SignUpWithGoogle or LoginWithGoogle
// - logout() → cookieクリア
// - token: Ref<string | null> — cookie 'auth-token' から読み取り
// - user: computed — JWTデコードして user_id, org_id, username 取得
// - isLoggedIn: computed
```

ログインRPCは公開パス（PUBLIC_PATHS）なのでJWT不要。
Connect RPC経由で `/api/grpc/logi.auth.AuthService/Login` を呼ぶ。

### 3-3. gRPC plugin — interceptor で x-auth-token 付与

- `plugins/grpc-client.client.ts` L16-18

```typescript
const transport = createGrpcWebTransport({
  baseUrl: '/api/grpc',
  interceptors: [
    (next) => async (req) => {
      const token = useCookie('auth-token').value
      if (token) {
        req.header.set('x-auth-token', token)
      }
      return next(req)
    },
  ],
})
```

### 3-4. ログイン画面

- `pages/login.vue`
- username/password フォーム（organization_id はデフォルト固定 or 選択）
- Google ログインボタン（将来）
- ログイン成功 → cookie にJWT保存 → `/` にリダイレクト

### 3-5. Nuxt middleware（ルートガード）

- `middleware/auth.ts`（グローバル or ページ指定）
- cookie にJWTがあればパス、なければ `/login` にリダイレクト
- JWTの有効期限チェック（exp claim）

### 3-6. logi-proto パッケージ更新

- `@yhonda-ohishi-pub-dev/logi-proto` には `auth.proto` の変更が既にpush済み
- フロントエンドで `npm update @yhonda-ohishi-pub-dev/logi-proto`
- `AuthService` を import して Login RPC を呼べる

### 3-7. デプロイ

```bash
cd nuxt-pwa-carins
NITRO_PRESET=cloudflare_module nuxt build && wrangler deploy
```

---

## Phase 4: 検証

1. https://nuxt-pwa-carins.mtamaramu.com/login にアクセス
2. username/password でログイン → JWT取得 → cookie保存
3. 車検証一覧が表示される（x-auth-token 付きリクエスト）
4. ログアウト → /login にリダイレクト
5. JWT無し（ログアウト状態）でも既存データは表示される（後方互換）

---

## 段階的移行

| 段階 | 認証 | 動作 |
|------|------|------|
| 現在 | JWT無し | DEFAULT_ORGANIZATION_ID でフォールバック |
| Phase 3完了後 | JWT有り | JWTのorg_idでRLS、マルチテナント対応 |
| 将来 | JWT必須化 | middleware のフォールバックを削除 |

## 注意事項

- `password_credentials` テーブルはRLS有効だがFORCEなし → ログイン時にorg context不要
- cf-grpc-proxy は全ヘッダーを転送する（`new Headers(request.headers)`）ので `x-auth-token` も転送される
- Nuxt catch-all proxy は明示的に転送するヘッダーを指定しているので追加が必要
- Google OAuth はフロントエンドに `GOOGLE_CLIENT_ID` を公開する必要がある（nuxt.config.ts の public）
