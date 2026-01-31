# ファイルと車検証の紐づき構造

## テーブル関係

```
files テーブル (GCS参照)            car_inspection_files_a (JSON紐づき)
+------------------------+         +------------------------------+
| uuid (PK)              |<--------| uuid (PK) = files.uuid       |
| filename               |         | type = 'application/json'    |
| s3_key (GCSパス)       |         | ElectCertMgNo                |
| created_at             |         | GrantdateE/Y/M/D             |
+------------------------+         +------------------------------+
                                            | 5フィールドで結合
                                   +------------------------------+
                                   | car_inspection               |
                                   | ElectCertMgNo                |
                                   | GrantdateE/Y/M/D             |
                                   +------------------------------+
                                            | 5フィールドで結合
+------------------------+         +------------------------------+
| files テーブル          |<--------| car_inspection_files_b (PDF)  |
| uuid (PK)              |         | uuid (PK) = files.uuid       |
| s3_key (GCSパス)       |         | type = 'application/pdf'     |
+------------------------+         +------------------------------+
```

## 紐づきキー

ファイル名ではなく、以下の5フィールドの完全一致で紐づく:

| フィールド | 説明 |
|-----------|------|
| `organization_id` | テナントID |
| `ElectCertMgNo` | 電子車検証管理番号 |
| `GrantdateE` | 交付日（元号） |
| `GrantdateY` | 交付日（年） |
| `GrantdateM` | 交付日（月） |
| `GrantdateD` | 交付日（日） |

## アップロードフロー

```
1. フロントエンド → CreateFile (files_service)
   - files テーブルに INSERT（uuid生成、GCSにバイナリ保存）
   - レスポンスで uuid を返す

2. フロントエンド → CreateCarInspectionFile (car_inspection_service)
   - car_inspection_files_a に INSERT（step 1 の uuid + ElectCertMgNo + Grantdate）
   - ON CONFLICT (uuid) DO UPDATE SET modified_at = NOW()
```

## ListCurrentCarInspections での紐づき取得

サブクエリで最新のファイルを取得:

```sql
-- PDF
(SELECT uuid::text FROM car_inspection_files_b
 WHERE organization_id = ci.organization_id
   AND "ElectCertMgNo" = ci."ElectCertMgNo"
   AND "GrantdateE" = ci."GrantdateE"
   AND "GrantdateY" = ci."GrantdateY"
   AND "GrantdateM" = ci."GrantdateM"
   AND "GrantdateD" = ci."GrantdateD"
   AND type = 'application/pdf'
   AND deleted_at IS NULL
 ORDER BY created_at DESC LIMIT 1) as pdf_uuid

-- JSON
(SELECT uuid::text FROM car_inspection_files_a
 WHERE ... AND type = 'application/json' AND deleted_at IS NULL
 ORDER BY created_at DESC LIMIT 1) as json_uuid
```

## 同一ファイル再アップロード時の挙動

毎回新しい UUID が生成されるため、同じファイルを複数回アップロードすると:

| 回 | files.uuid | s3_key | car_inspection_files | 紐づき状態 |
|----|-----------|--------|---------------------|-----------|
| 1回目 | `aaa...` | `{org}/aaa...` | uuid=`aaa...` | 孤立（古い） |
| 2回目 | `bbb...` | `{org}/bbb...` | uuid=`bbb...` | **使用される** |

- `ORDER BY created_at DESC LIMIT 1` により**最新のファイルのみ**が紐づく
- 古いファイルは `files` テーブル、`car_inspection_files_a/b` テーブル、GCS 全てに残り続ける
- 古いファイルの `deleted_at` は NULL のまま（自動 soft delete なし）

## 注意事項

- 重複ファイルのクリーンアップ機構は未実装（孤立ファイルが蓄積する）
- `car_inspection_files_a` にはPDF/JSON両方が INSERT される（`create_car_inspection_file` の実装）
- `ListCurrentCarInspections` は PDF を `_b` テーブル、JSON を `_a` テーブルから検索する
