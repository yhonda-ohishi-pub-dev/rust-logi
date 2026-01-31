# /api/cam (createCam) → rust-logi 移行計画

hono-logiのカメラSD同期+Flickrアップロード処理をrust-logiのgRPCメソッドとして移植する。

## 現状

- **hono-logi**: `GET /api/cam` → `createCamHandler` (createCam.ts, ~500行)
- **rust-logi**: cam_filesの参照系のみ (ListCamFiles, ListCamFileDates, CreateCamFileExe)
- **cron-worker**: Cloudflare Workerから定期実行

## 処理フロー (hono-logi)

```
1. DB最終レコードのdate取得 → 開始日決定
2. カメラSDカード → Digest認証 → XMLディレクトリ探索
   sdcardCgi/{machineName}/Event → dates
   sdcardCgi/{machineName}/Event/{date} → hours
   sdcardCgi/{machineName}/Event/{date}/{hour} → files
3. cam_files テーブルに UPSERT
4. flickr_id NULL のファイルをカメラからダウンロード
5. Flickr API でアップロード → flickr_id を UPDATE
```

## チェックリスト

### Phase 1: 環境変数・設定

- [ ] `src/config.rs` に追加:
  - `CAM_DIGEST_USER` — カメラ Digest認証ユーザー名
  - `CAM_DIGEST_PASS` — カメラ Digest認証パスワード
  - `CAM_MACHINE_NAME` — カメラ名 (例: `TS-NA230WP-48`)
  - `CAM_SDCARD_CGI` — SDカードCGI URL
  - `CAM_MP4_CGI` — MP4ダウンロード URL
  - `CAM_JPG_CGI` — JPGダウンロード URL
- [ ] `.env` にサンプル値追加

### Phase 2: Proto定義

- [ ] `cam_files.proto` に `SyncCamFiles` RPC 追加:
  ```protobuf
  rpc SyncCamFiles(SyncCamFilesRequest) returns (SyncCamFilesResponse);

  message SyncCamFilesRequest {}
  message SyncCamFilesResponse {
    int32 processed_dates = 1;
    int32 processed_hours = 2;
    int32 processed_files = 3;
    int32 uploaded_to_flickr = 4;
    int32 upload_errors = 5;
  }
  ```

### Phase 3: Digest認証

- [ ] `Cargo.toml` に `digest-auth` クレート追加 (or 自前実装)
- [ ] `src/services/cam_files_service.rs` に Digest認証ヘルパー追加
  - hono-logi: MD5自前実装 + www-authenticate パース
  - rust: `digest-auth` クレートで簡潔に実装可能
  - 401 → nonce取得 → ha1/ha2計算 → Authorizationヘッダー生成

### Phase 4: カメラXML探索

- [ ] `Cargo.toml` に `quick-xml` クレート追加
- [ ] ディレクトリ探索関数:
  - `fetch_dates(base_url, machine, after_date)` → `Vec<String>`
  - `fetch_hours(base_url, machine, date, after_hour)` → `Vec<(String, String)>`
  - `fetch_files(base_url, machine, date, hour)` → `Vec<String>`
- [ ] XML `<Dir Name="...">` と `<Name>...</Name>` パーサー
- [ ] `_!` を含むファイル名はスキップ

### Phase 5: DB操作

- [ ] cam_filesへのUPSERT:
  ```sql
  INSERT INTO cam_files (name, organization_id, date, hour, type, cam)
  VALUES ($1, $2::uuid, $3, $4, $5, $6)
  ON CONFLICT (organization_id, name) DO UPDATE SET
    date = EXCLUDED.date, hour = EXCLUDED.hour,
    type = EXCLUDED.type, cam = EXCLUDED.cam
  ```
- [ ] 最終レコード取得: `SELECT * FROM cam_files ORDER BY name DESC LIMIT 1`
- [ ] flickr_id NULL のファイル取得:
  ```sql
  SELECT * FROM cam_files
  WHERE date >= $1 AND flickr_id IS NULL
  LIMIT 100
  ```

### Phase 6: Flickrアップロード

- [ ] Flickr upload API 実装 (OAuth 1.0a署名は既存コード再利用)
  - エンドポイント: `https://up.flickr.com/services/upload/`
  - multipart/form-data でファイル送信
  - パラメータ: `title={filename}`, `tags=upBySytem`
  - レスポンスXML: `<photoid>123456</photoid>`
- [ ] アップロード後に cam_files の flickr_id を UPDATE
- [ ] `tokio::spawn` でバックグラウンド実行（hono-logiの`waitUntil`相当）

### Phase 7: SyncCamFiles RPC実装

- [ ] `CamFilesServiceImpl` に `sync_cam_files` メソッド追加
- [ ] フロー:
  1. RLS設定
  2. カメラ設定チェック (config.as_ref())
  3. DB最終レコード取得
  4. Digest認証でXMLディレクトリ探索
  5. 新ファイルをcam_filesにUPSERT
  6. flickr_id NULLのファイルをダウンロード→Flickrアップロード (background)
  7. 統計情報をレスポンスで返却
- [ ] エラーハンドリング: 個別ファイルの失敗はログして継続

### Phase 8: ビルド・テスト

- [ ] `cargo build --release`
- [ ] ローカルテスト (カメラ接続可能な環境で)
- [ ] Cloud Runデプロイ
- [ ] cron-workerから `SyncCamFiles` 呼び出しテスト

### Phase 9: cron-worker更新

- [ ] `@connectrpc/connect-web` で gRPC-Web呼び出し:
  ```typescript
  import { CamFilesService, FlickrService } from "@yhonda-ohishi-pub-dev/logi-proto";

  export default {
    async scheduled(event, env, ctx) {
      const transport = createGrpcWebTransport({ baseUrl: env.RUST_LOGI_URL });

      // 1. カメラSD同期 + Flickrアップロード
      const camClient = createClient(CamFilesService, transport);
      const syncResult = await camClient.syncCamFiles({});

      // 2. flickr_photo メタデータインポート
      const flickrClient = createClient(FlickrService, transport);
      const importResult = await flickrClient.importFlickrPhotos({ limit: 500 });
    },
  };
  ```
- [ ] hono-logiへの呼び出しを削除

## 依存クレート追加

| クレート | 用途 |
|----------|------|
| `digest-auth` | カメラ Digest認証 |
| `quick-xml` | カメラXMLレスポンスパース |

※ `reqwest`, `ring`, `base64` は既存

## 環境変数 (追加分)

```bash
CAM_DIGEST_USER=admin
CAM_DIGEST_PASS=xxxxx
CAM_MACHINE_NAME=TS-NA230WP-48
CAM_SDCARD_CGI=http://xxx.xxx.xxx.xxx/cgi-bin/sdcard.cgi?action=list&path=
CAM_MP4_CGI=http://xxx.xxx.xxx.xxx/cgi-bin/download.cgi?path=
CAM_JPG_CGI=http://xxx.xxx.xxx.xxx/cgi-bin/download.cgi?path=
```

## 注意事項

- Cloud RunからカメラへのアクセスはVPN/Cloud VPN経由が必要（カメラはLAN内）
  - 現在hono-logi (Cloudflare Workers) からアクセスできているなら、同じネットワーク経路を確認
- Flickrアップロードのタグ `upBySytem` はhono-logiのタイポだが互換性のため維持
- カメラXMLパースの `_!` フィルタはカメラ一時ファイル除外用
