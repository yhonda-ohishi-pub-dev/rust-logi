# rust-logi / rust-alc-api 関係図

## 全体構成

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Supabase PostgreSQL                         │
│                    (tvbjvhvslgdwwlhpkezh)                          │
│                                                                     │
│  ┌──────────────────────┐    ┌────────────────────────────────┐    │
│  │    logi スキーマ      │    │      alc_api スキーマ          │    │
│  │                      │    │                                │    │
│  │  organizations       │    │  tenants (+ slug)              │    │
│  │  app_users           │    │  users (google_sub/lineworks)  │    │
│  │  user_organizations  │    │  employees                    │    │
│  │  oauth_accounts      │    │  measurements                 │    │
│  │  dtakologs           │    │  tenko_*                      │    │
│  │  cam_files           │    │  devices                      │    │
│  │  flickr_*            │    │  timecard_*                   │    │
│  │  kudg*               │    │  webhook_*                    │    │
│  │  dvr_notifications   │    │  equipment_failures           │    │
│  │  ichiban_cars        │    │  sso_provider_configs ←移動済  │    │
│  │  items               │    │                                │    │
│  │  bot_configs         │    │  car_inspection      ←移動済  │    │
│  │  roles/permissions   │    │  car_inspection_*    ←移動済  │    │
│  │  access_requests     │    │  files / files_append ←移動済  │    │
│  │  ...                 │    │  file_access_logs    ←移動済  │    │
│  │                      │    │  car_inspection_nfc_tags ←移動 │    │
│  │  role: rust_logi_app │    │  pending_car_inspection_pdfs   │    │
│  │  search_path: logi   │    │                                │    │
│  └──────────────────────┘    │  role: alc_api_app             │    │
│                              │  search_path: alc_api          │    │
│                              └────────────────────────────────┘    │
│                                                                     │
│  RLS: COALESCE(current_tenant_id, current_organization_id)         │
│       → 移動済みテーブルは両方のアプリからアクセス可能               │
└─────────────────────────────────────────────────────────────────────┘
```

## バックエンド

```
┌─────────────────────────┐     ┌─────────────────────────┐
│      rust-logi           │     │      rust-alc-api        │
│   (Cloud Run)            │     │   (Cloud Run)            │
│                          │     │                          │
│  Protocol: gRPC-Web      │     │  Protocol: REST (Axum)   │
│  Framework: tonic         │     │  Framework: Axum         │
│  Port: 8080              │     │  Port: 8080              │
│                          │     │                          │
│  Services:               │     │  Routes:                 │
│   DtakologsService       │     │   /api/auth/*            │
│   CamFilesService        │     │   /api/employees/*       │
│   FlickrService          │     │   /api/measurements/*    │
│   DvrNotificationsService│     │   /api/tenko-*/*         │
│   ItemsService           │     │   /api/devices/*         │
│   AuthService            │     │   /api/timecard/*        │
│   MemberService          │     │   /api/car-inspections/* │
│   OrganizationService    │     │   /api/car-inspection-*  │
│   BotConfigService       │     │   /api/files/*           │
│   AccessRequestService   │     │   /api/nfc-tags/*        │
│   HealthService          │     │   /api/upload/*          │
│                          │     │                          │
│  Storage: GCS            │     │  Storage:                │
│   bucket: rust-logi-files│     │   R2: alc-face-photos    │
│   (DVR mp4 等)           │     │   R2: carins-files       │
└─────────────────────────┘     └─────────────────────────┘
```

## フロントエンド → バックエンド

```
┌──────────────────────┐
│   nuxt-pwa-carins     │  車検証管理 PWA
│   (CF Workers)        │
│                       │    REST
│   carins.mtamaramu.com├──────────→ rust-alc-api
│                       │            /api/car-inspections/*
│                       │            /api/files/*
│                       │            /api/nfc-tags/*
└──────────────────────┘

┌──────────────────────┐
│   nuxt-dtako-logs     │  DTako ログビューワー
│   (CF Workers)        │
│                       │    gRPC-Web
│                       ├──────────→ rust-logi (via cf-grpc-proxy)
│                       │            DtakologsService
│                       │            CamFilesService
└──────────────────────┘

┌──────────────────────┐
│   nuxt-items          │  物品管理
│   (CF Workers)        │
│                       │    gRPC-Web
│   items.mtamaramu.com ├──────────→ rust-logi (via cf-grpc-proxy)
│                       │            ItemsService
└──────────────────────┘

┌──────────────────────┐
│   alc-app             │  アルコールチェック
│   (CF Workers)        │
│                       │    REST
│                       ├──────────→ rust-alc-api
│                       │            /api/employees/*
│                       │            /api/measurements/*
│                       │            /api/tenko-*/*
│                       │            /api/devices/*
└──────────────────────┘
```

## 認証フロー

```
┌──────────────────────┐
│    auth-worker        │  JWT 発行 (CF Workers)
│  auth.mtamaramu.com   │
│                       │
│  LINE WORKS OAuth ────┤──→ rust-logi AuthService (gRPC)
│  Google OAuth ────────┤      → JWT 発行 (rust-logi JWT_SECRET)
│                       │
│  JWT クレーム:         │
│   sub: user_id        │
│   org: organization_id│  ← tenant_id と同じ UUID
│   org_slug            │
│   provider            │
└───────┬───────────────┘
        │ JWT
        ▼
┌──────────────────────┐     ┌─────────────────────────┐
│  nuxt-pwa-carins      │     │  nuxt-dtako-logs         │
│  nuxt-items           │     │                          │
│                       │     │  x-auth-token ヘッダー    │
│  プロキシで変換:       │     │  → rust-logi で検証       │
│  JWT.org → X-Tenant-ID│     └─────────────────────────┘
│  → rust-alc-api       │
└──────────────────────┘

┌──────────────────────┐
│  alc-app              │  独自認証
│                       │
│  Google OAuth ────────┤──→ rust-alc-api /api/auth/google
│                       │      → JWT 発行 (rust-alc-api JWT_SECRET)
│  X-Tenant-ID ─────────┤──→ キオスクモード (JWT 不要)
└──────────────────────┘
```

## ストレージ

| バケット | 種類 | 用途 | アクセス元 |
|---------|------|------|-----------|
| `rust-logi-files` (GCS) | GCS Autoclass | DVR mp4、レガシーファイル | rust-logi |
| `carins-files` (R2) | Cloudflare R2 | 車検証 PDF/JSON (GCS から移行済み) | rust-alc-api |
| `alc-face-photos` (R2) | Cloudflare R2 | 顔写真 | rust-alc-api |

## テナント / 組織の対応

```
rust-logi:   organization_id = "00000000-0000-0000-0000-000000000001"
rust-alc-api: tenant_id      = "00000000-0000-0000-0000-000000000001"
                                 ↑ 同じ UUID
```

- 1社（大石運輸倉庫）のみ運用中
- auth-worker の JWT `org` クレーム = rust-alc-api の `tenant_id`
- 移動済みテーブルの RLS は `COALESCE(current_tenant_id, current_organization_id)` で両方対応

## 今後の移行候補

rust-logi → rust-alc-api への段階的移行パターン:

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
-- 4. 権限
GRANT ALL ON alc_api.<table> TO alc_api_app;
```

| 移行候補 | テーブル数 | 関連フロントエンド |
|---------|-----------|-----------------|
| dtakologs + cam_files + flickr | ~5 | nuxt-dtako-logs |
| items | 1 | nuxt-items |
| kudg* | 6 | nuxt-pwa-carins (未使用?) |
| DVR notifications | 1 | - |
| 認証統合 (organizations → tenants) | ~8 | 全フロントエンド |
