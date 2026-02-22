# JWT認証 全サービス テスト計画

## 前提: テスト用JWTトークン取得

```bash
# auth-worker 経由でJWTを取得
# 302リダイレクトのURLフラグメントからトークンを抽出
curl -s -o /dev/null -D - -X POST "https://auth.mtamaramu.com/auth/login" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "organization_id=00000000-0000-0000-0000-000000000001&username=<USER>&password=<PASS>&redirect_uri=https://nuxt-pwa-carins.mtamaramu.com" \
  | grep -i location
# → Location: https://...#token=<JWT>&expires_at=...&org_id=...

# トークンを変数に保存
TOKEN="eyJ..."
ORG_ID="00000000-0000-0000-0000-000000000001"
```

---

## 1. cf-grpc-proxy

**確認項目**: JWT検証ゲート + x-auth-token 転送

### 1-1. 有効なJWTでリクエスト → 成功
```bash
curl -s -X POST "https://cf-grpc-proxy.m-tama-ramu.workers.dev/logi.car_inspection.CarInspectionService/ListCurrentCarInspections" \
  -H "Content-Type: application/json" \
  -H "Connect-Protocol-Version: 1" \
  -H "x-auth-token: $TOKEN" \
  -d '{}'
# 期待: 200 + 車検証データ
```

### 1-2. 無効なJWTでリクエスト → 401
```bash
curl -s -X POST "https://cf-grpc-proxy.m-tama-ramu.workers.dev/logi.car_inspection.CarInspectionService/ListCurrentCarInspections" \
  -H "Content-Type: application/json" \
  -H "Connect-Protocol-Version: 1" \
  -H "x-auth-token: invalid-token" \
  -d '{}'
# 期待: 401 {"error": "Invalid or expired token"}
```

### 1-3. JWTなしでリクエスト → 移行期間中は通過（警告のみ）
```bash
curl -s -X POST "https://cf-grpc-proxy.m-tama-ramu.workers.dev/logi.car_inspection.CarInspectionService/ListCurrentCarInspections" \
  -H "Content-Type: application/json" \
  -H "Connect-Protocol-Version: 1" \
  -d '{}'
# 期待: 200（後方互換、ログに警告）
```

### 1-4. 公開パスはJWT不要
```bash
curl -s -X POST "https://cf-grpc-proxy.m-tama-ramu.workers.dev/grpc.health.v1.Health/Check" \
  -H "Content-Type: application/json" \
  -H "Connect-Protocol-Version: 1" \
  -d '{}'
# 期待: 200 {"status": "SERVING"}
```

---

## 2. rust-logi (Cloud Run)

**確認項目**: x-auth-token からJWT検証 → AuthenticatedUser 設定

### 2-1. GCP IAMトークン + x-auth-token でリクエスト
```bash
IAM_TOKEN=$(gcloud auth print-identity-token)

# 車検証一覧（既存サービス）
grpcurl -H "Authorization: Bearer $IAM_TOKEN" \
  -H "x-auth-token: $TOKEN" \
  -H "x-organization-id: $ORG_ID" \
  -d '{}' \
  rust-logi-747065218280.asia-northeast1.run.app:443 \
  logi.car_inspection.CarInspectionService/ListCurrentCarInspections 2>&1 | head -5
# 期待: 車検証データが返る
```

### 2-2. OrganizationService（AuthenticatedUser必須）
```bash
grpcurl -H "Authorization: Bearer $IAM_TOKEN" \
  -H "x-auth-token: $TOKEN" \
  -d '{}' \
  rust-logi-747065218280.asia-northeast1.run.app:443 \
  logi.organization.OrganizationService/ListMyOrganizations
# 期待: ユーザーの組織一覧が返る（Unauthenticated にならない）
```

### 2-3. MemberService（AuthenticatedUser必須）
```bash
grpcurl -H "Authorization: Bearer $IAM_TOKEN" \
  -H "x-auth-token: $TOKEN" \
  -H "x-organization-id: $ORG_ID" \
  -d '{}' \
  rust-logi-747065218280.asia-northeast1.run.app:443 \
  logi.member.MemberService/ListMembers
# 期待: メンバー一覧が返る（Unauthenticated にならない）
```

### 2-4. x-auth-token なし → 後方互換（パススルー）
```bash
grpcurl -H "Authorization: Bearer $IAM_TOKEN" \
  -H "x-organization-id: $ORG_ID" \
  -d '{}' \
  rust-logi-747065218280.asia-northeast1.run.app:443 \
  logi.car_inspection.CarInspectionService/ListCurrentCarInspections 2>&1 | head -5
# 期待: デフォルトorg_idで車検証データが返る
```

---

## 3. nuxt-pwa-carins

**確認項目**: ログイン → JWT付きリクエスト → データ表示

### 3-1. ログインリダイレクト
```
ブラウザで https://nuxt-pwa-carins.mtamaramu.com にアクセス
→ auth-worker のログイン画面にリダイレクトされる
→ Google ログインまたはユーザー名/パスワードでログイン
→ アプリにリダイレクト（URL # にトークン付き）
```

### 3-2. ログイン後のデータ表示
```
ログイン後:
→ 車検証一覧が表示される
→ DevTools > Network で /api/grpc/* リクエストに x-auth-token ヘッダーが付いている
```

### 3-3. ページリロード
```
F5でリロード
→ localStorage からトークン復元
→ データが引き続き表示される（再ログイン不要）
```

### 3-4. ログアウト
```
ログアウト操作
→ localStorage + cookie クリア
→ ログイン画面にリダイレクト
```

---

## 4. nuxt-dtako-logs

**確認項目**: nuxt-pwa-carins と同様のJWT認証フロー

### 4-1. ログインリダイレクト
```
ブラウザで https://nuxt-dtako-logs.mtamaramu.com にアクセス
→ auth-worker にリダイレクト → ログイン → アプリに戻る
```

### 4-2. DTakoログ表示
```
ログイン後:
→ DTakoログ一覧が表示される
→ DevTools で x-auth-token ヘッダー確認
```

---

## 5. smb-upload-worker

**確認項目**: Authorization: Bearer → x-auth-token 変換

### 5-1. ファイルアップロード
```bash
curl -X POST "https://smb-upload-worker.m-tama-ramu.workers.dev/upload" \
  -H "Authorization: Bearer $TOKEN" \
  -H "x-organization-id: $ORG_ID" \
  -F "data=@test.pdf" \
  -v
# 期待: {"uuid": "...", "message": "Uploaded test.pdf"}
```

### 5-2. JWT なし → 401
```bash
curl -s -X POST "https://smb-upload-worker.m-tama-ramu.workers.dev/upload" \
  -H "x-organization-id: $ORG_ID" \
  -F "data=@test.pdf"
# 期待: 401 "Missing or invalid Authorization header"
```

---

## チェックリスト

| # | テスト | 期待結果 | 結果 |
|---|--------|----------|------|
| 1-1 | cf-grpc-proxy: 有効JWT | 200 + データ | |
| 1-2 | cf-grpc-proxy: 無効JWT | 401 | |
| 1-3 | cf-grpc-proxy: JWT なし | 200（移行期間） | |
| 1-4 | cf-grpc-proxy: 公開パス | 200 | |
| 2-1 | rust-logi: 車検証一覧 | データ返却 | |
| 2-2 | rust-logi: ListMyOrganizations | 組織一覧 | |
| 2-3 | rust-logi: ListMembers | メンバー一覧 | |
| 2-4 | rust-logi: JWT なし | 後方互換で動作 | |
| 3-1 | nuxt-pwa-carins: ログイン | リダイレクト成功 | |
| 3-2 | nuxt-pwa-carins: データ表示 | 車検証表示 | |
| 3-3 | nuxt-pwa-carins: リロード | トークン維持 | |
| 3-4 | nuxt-pwa-carins: ログアウト | ログイン画面へ | |
| 4-1 | nuxt-dtako-logs: ログイン | リダイレクト成功 | |
| 4-2 | nuxt-dtako-logs: データ表示 | ログ表示 | |
| 5-1 | smb-upload-worker: アップロード | uuid 返却 | |
| 5-2 | smb-upload-worker: JWT なし | 401 | |
