# S3 Infrastructure Setup

## バケット作成

```bash
aws s3 mb s3://rust-logi-files --region ap-northeast-1
```

## Lifecycle Policy適用

```bash
aws s3api put-bucket-lifecycle-configuration \
  --bucket rust-logi-files \
  --lifecycle-configuration file://infra/s3-lifecycle-policy.json
```

## Lifecycle Policy確認

```bash
aws s3api get-bucket-lifecycle-configuration --bucket rust-logi-files
```

## ストレージクラス移行ルール

| 期間 | ストレージクラス | 説明 |
|-----|----------------|------|
| 0-30日 | STANDARD | 頻繁にアクセスされるファイル |
| 30-180日 | STANDARD_IA | アクセス頻度が低いファイル（取得コストあり） |
| 180日以降 | GLACIER | アーカイブ（復元に3-5時間必要） |

## Glacier復元オプション

| Tier | 復元時間 | コスト |
|------|---------|--------|
| Expedited | 1-5分 | 高 |
| Standard | 3-5時間 | 中 |
| Bulk | 5-12時間 | 低 |

## アクセス時のStandard復元

アプリケーションでは、ファイルがダウンロードされた際に自動的にStandardストレージクラスに戻す機能が実装されています。これにより、再度アクセスされる可能性のあるファイルはより高速にアクセスできるようになります。
