# browser-render-rust → rust-logi PostgreSQL 直接挿入 計画

## 背景・問題

- **現状**: browser-render-rust → Hono API (Cloudflare Workers) → D1
- **問題**: Hono APIの`forEach + async`バグでD1へのデータ保存が失敗
- **証拠**: D1最新データは2026-01-23 16:40、今日（2026-01-24）のPOSTは200を返すがDBに未反映

## 解決策

browser-render-rustからrust-logiのPostgreSQLに直接データを挿入する

## アーキテクチャ

```
現在:
browser-render-rust → POST → hono-api (CF Workers) → D1 (失敗)

変更後:
browser-render-rust → gRPC → rust-logi → PostgreSQL (Cloud SQL)
                                      ↓
                              dtakologsテーブル（RLS対応）
```

## 実装計画

### Phase 1: rust-logi側 - BulkCreate RPC追加

**ファイル**: `src/services/dtakologs_service.rs`

既存のDtakologsServiceに`BulkCreate` RPCを追加：

```protobuf
// packages/logi-proto/proto/dtakologs.proto に追加
rpc BulkCreate(BulkCreateDtakologsRequest) returns (BulkCreateDtakologsResponse);

message BulkCreateDtakologsRequest {
  repeated Dtakolog dtakologs = 1;
}

message BulkCreateDtakologsResponse {
  bool success = 1;
  int32 records_added = 2;
  int32 total_records = 3;
  string message = 4;
}
```

**実装内容**:
- バッチINSERT（ON CONFLICT DO UPDATE）
- RLS対応（organization_id設定）
- トランザクション処理

### Phase 2: browser-render-rust側 - gRPCクライアント追加

**ファイル**: `browser-render-rust/src/browser/renderer.rs`

1. rust-logiのprotoファイルを追加
2. `send_to_rust_logi()` メソッド追加
3. `send_raw_to_hono_api()` を削除

**依存関係追加** (Cargo.toml):
```toml
tonic = "0.12"
prost = "0.13"
```

### Phase 3: 設定・環境変数

**browser-render-rust**:
```env
# 必須: rust-logiのURL（デフォルト値なし）
RUST_LOGI_URL=https://rust-logi-XXXXX.run.app

# 必須: 組織ID（デフォルト値なし）
RUST_LOGI_ORGANIZATION_ID=<your-organization-uuid>
```

**注**: 両方の環境変数が設定されていない場合、警告ログが出力されます。

### Phase 4: デプロイ

1. rust-logi: BulkCreate RPC追加 → Cloud Runデプロイ
2. browser-render-rust: gRPCクライアント追加 → **GCEデプロイ（現状維持）**

**注**: browser-render-rustはChrome動作の安定性のためGCEのまま運用

## 修正対象ファイル

### rust-logi
| ファイル | 変更内容 |
|---------|---------|
| `packages/logi-proto/proto/dtakologs.proto` | BulkCreate RPC定義追加 |
| `src/services/dtakologs_service.rs` | BulkCreate実装 |

### browser-render-rust
| ファイル | 変更内容 |
|---------|---------|
| `Cargo.toml` | tonic, prost依存追加 |
| `build.rs` | rust-logi proto コンパイル追加 |
| `proto/logi.proto` | rust-logiのdtakologs.proto コピー |
| `src/browser/renderer.rs` | gRPCクライアント追加、`send_raw_to_hono_api()`を`send_to_rust_logi()`に置換 |
| `src/config.rs` | RUST_LOGI_URL設定追加、HONO_API_URL削除 |

## 検証方法

1. **ローカルテスト**:
   ```bash
   # rust-logi起動
   cd rust-logi && ./start.sh

   # browser-render-rust起動（RUST_LOGI_URL=localhost:50051）
   cd browser-render-rust && cargo run

   # データ取得トリガー
   curl http://localhost:8080/v1/vehicle/data

   # PostgreSQL確認
   psql -c "SELECT COUNT(*) FROM dtakologs WHERE data_date_time > '2026-01-24'"
   ```

2. **Cloud Run確認**:
   ```bash
   # gRPCヘルスチェック
   grpcurl -d '{}' rust-logi-XXX.run.app:443 logi.DtakologsService/BulkCreate

   # D1との比較
   # D1: 733万件、最新2026-01-23
   # PostgreSQL: 最新2026-01-24になっていること
   ```

## 代替案（検討済み）

1. **Hono APIを修正**: forEach→for...of に変更
   - メリット: 最小変更
   - デメリット: Cloudflare Workers + D1の制約は残る

2. **browser-render-rustから直接PostgreSQL接続**:
   - メリット: rust-logi変更不要
   - デメリット: DB認証情報をbrowser-render-rustに持たせる必要

→ **gRPC経由が最もクリーン**（認証・RLS・バリデーションをrust-logiに集約）

## スケジュール

1. Phase 1: rust-logi BulkCreate RPC実装
2. Phase 2: browser-render-rust gRPCクライアント追加
3. Phase 3: ローカルテスト
4. Phase 4: デプロイ・本番テスト
