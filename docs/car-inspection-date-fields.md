# 車検証（car_inspection）日付フィールドの仕様

## 概要

車検証JSONファイル内の日付フィールドには先頭スペースが含まれている。
これは元データの仕様であり、そのまま保存する設計。

## スペースを含む日付フィールド

| フィールド | 例 | 備考 |
|-----------|-----|------|
| `ElectCertPublishdateY` | `" 5"` | 電子証明書発行年 |
| `ElectCertPublishdateM` | `" 7"` | 電子証明書発行月 |
| `ElectCertPublishdateD` | `" 6"` | 電子証明書発行日 |
| `GrantdateY` | `" 5"` | 交付年 |
| `GrantdateM` | `" 7"` | 交付月 |
| `GrantdateD` | `" 6"` | 交付日 |
| `ValidPeriodExpirdateY` | `" 6"` | 有効期限年 |
| `ValidPeriodExpirdateM` | `" 7"` | 有効期限月 |
| `ValidPeriodExpirdateD` | `" 9"` | 有効期限日 |

## DB設計

### car_inspectionテーブル

- 全日付フィールドは **TEXT型**
- スペースを含む値をそのまま保存
- 一意キー: `(organization_id, "ElectCertMgNo", "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD")`

### 有効期限判定

有効期限の判定には `TwodimensionCodeInfoValidPeriodExpirdate` を使用:
- 形式: `"240709"` (YYMMDD、スペースなし)
- SQL比較: `>= to_char(CURRENT_DATE, 'YYYYMMDD')`

```sql
WHERE ci."TwodimensionCodeInfoValidPeriodExpirdate" >= to_char(CURRENT_DATE, 'YYYYMMDD')
```

## INSERT/UPDATE処理

### Upsert方式

```sql
INSERT INTO car_inspection (...)
VALUES (...)
ON CONFLICT (organization_id, "ElectCertMgNo", "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD")
DO UPDATE SET modified_at = NOW()
```

### 関連ファイル

- `src/services/car_inspection_service.rs` - Insert/Update処理 (Lines 151-314)
- `migrations/00003_create_car_inspection.sql` - テーブル定義

## car_inspection_filesとのJOIN

`car_inspection_files_a` (JSON用) と `car_inspection_files_b` (PDF用) は
Grantdate系フィールドで結合:

```sql
SELECT ...
FROM car_inspection ci
WHERE EXISTS (
    SELECT 1 FROM car_inspection_files_a
    WHERE "ElectCertMgNo" = ci."ElectCertMgNo"
      AND "GrantdateE" = ci."GrantdateE"
      AND "GrantdateY" = ci."GrantdateY"
      AND "GrantdateM" = ci."GrantdateM"
      AND "GrantdateD" = ci."GrantdateD"
)
```

## データ移行時の注意

### convert_and_import.py

pg_dumpからデータをインポートする際、以下の処理を行う:

1. **car_inspection**: スペースなしの重複レコードをスキップ
   - `GrantdateY`が1桁でスペースなし（例: `"5"`）→ スキップ
   - `GrantdateY`が1桁でスペースあり（例: `" 5"`）→ 取り込む
   - `GrantdateY`が2桁（例: `"12"`）→ 取り込む

2. **car_inspection_files_a/b**: インポート後にGrantdateにスペースを追加
   - 旧hono-logiではスペースなしで保存されていた
   - `fix_car_inspection_files_grantdate()`で1桁の値にスペースを追加

### 重複データの原因

過去にhono-logiで同じ車検証が2回登録されたケースがあり:
- 1回目: `" 5"` (スペース付き) ← 正しい形式
- 2回目: `"5"` (スペースなし) ← 重複

これらはUpsertキーが異なるため別レコードとして保存されていた。

## 注意事項

1. **一貫性**: スペースの有無が異なるデータが混在すると重複レコードが発生する可能性
2. **外部連携**: 他システムとデータ連携する際はスペース処理の一貫性に注意
3. **トリム禁止**: 現在の設計ではトリムせずそのまま保存する仕様
4. **JOIN条件**: `car_inspection`と`car_inspection_files_a/b`のGrantdateは完全一致が必要
