# kudguri関連データのGCS移行計画

## 概要

`kudguri`テーブルをマスターとして残し、関連テーブル（`kudgivt`, `kudgcst`, `kudgfry`, `kudgful`, `kudgsir`）のデータをGCSにJSON形式で保存する。

## 現状

| テーブル | レコード数 | サイズ | 備考 |
|----------|------------|--------|------|
| kudguri  | 196        | 208 KB | マスター（残す） |
| kudgivt  | 4,247      | 1.9 MB | GCSへ移行 |
| kudgcst  | 3          | 64 KB  | GCSへ移行 |
| kudgfry  | 20         | 64 KB  | GCSへ移行 |
| kudgful  | 270        | 160 KB | GCSへ移行 |
| kudgsir  | 745        | 424 KB | GCSへ移行 |

## GCS保存構造

```
gs://bucket/{org_id}/kudguri/{kudguri_uuid}/
  ├── ivt.json   (kudgivt - 運行データ)
  ├── cst.json   (kudgcst)
  ├── fry.json   (kudgfry)
  ├── ful.json   (kudgful)
  └── sir.json   (kudgsir)
```

各JSONファイルは配列形式:
```json
[
  { "uuid": "xxx", "unkou_no": "...", ... },
  { "uuid": "yyy", "unkou_no": "...", ... }
]
```

## アーカイブメソッド定義（Rustコード）

```rust
// src/archive/methods.rs

pub struct ArchiveMethod {
    pub id: &'static str,
    pub parent_table: &'static str,
    pub parent_pk_type: PkType,
    pub child_tables: &'static [ChildTable],
    pub storage_path_template: &'static str,  // "{org_id}/kudguri/{parent_pk}/"
    pub retention_days: i32,
}

pub struct ChildTable {
    pub table: &'static str,
    pub fk_column: &'static str,
    pub file: &'static str,
}

pub enum PkType { Uuid, Bigint, Text }

// 定義
pub static KUDGURI_METHOD: ArchiveMethod = ArchiveMethod {
    id: "kudguri",
    parent_table: "kudguri",
    parent_pk_type: PkType::Uuid,
    child_tables: &[
        ChildTable { table: "kudgivt", fk_column: "kudguri_uuid", file: "ivt.json" },
        ChildTable { table: "kudgcst", fk_column: "kudguri_uuid", file: "cst.json" },
        ChildTable { table: "kudgfry", fk_column: "kudguri_uuid", file: "fry.json" },
        ChildTable { table: "kudgful", fk_column: "kudguri_uuid", file: "ful.json" },
        ChildTable { table: "kudgsir", fk_column: "kudguri_uuid", file: "sir.json" },
    ],
    storage_path_template: "{org_id}/kudguri/{parent_pk}/",
    retention_days: 90,
};

pub static METHODS: &[&ArchiveMethod] = &[&KUDGURI_METHOD];

// 実行時のパス生成例:
// org_id = "abc-123", parent_pk = "def-456" (kudguri.uuid) の場合
// → "abc-123/kudguri/def-456/ivt.json"
// → "abc-123/kudguri/def-456/cst.json"
// → ...
```

## アーカイブ追跡テーブル（親単位で1レコード）

```sql
-- アーカイブジョブ（親テーブル単位で管理）
CREATE TABLE data_archives (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),

    -- アーカイブ対象（メソッドIDはRust側で定義）
    method_id TEXT NOT NULL,                  -- 'kudguri', 'orders', etc.
    parent_pk_uuid UUID,
    parent_pk_bigint BIGINT,
    parent_pk_text TEXT,

    -- ストレージ情報
    storage_type TEXT NOT NULL DEFAULT 'gcs',
    storage_base_path TEXT NOT NULL,          -- 'org-uuid/kudguri/parent-uuid/'
    storage_class TEXT,

    -- 各ファイルの情報
    files JSONB NOT NULL DEFAULT '[]',        -- [{"file": "ivt.json", "checksum": "...", "byte_size": 123, "record_count": 45}, ...]

    -- ライフサイクル
    status TEXT NOT NULL DEFAULT 'scheduled', -- scheduled, pending, archived, verified, restored, deleted
    scheduled_at TIMESTAMPTZ NOT NULL,
    archived_at TIMESTAMPTZ,
    verified_at TIMESTAMPTZ,
    source_deleted_at TIMESTAMPTZ,
    restored_at TIMESTAMPTZ,

    -- メタデータ
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ユニーク制約（メソッド + 親PK）
CREATE UNIQUE INDEX idx_data_archives_method_uuid
  ON data_archives(method_id, parent_pk_uuid)
  WHERE parent_pk_uuid IS NOT NULL;

CREATE UNIQUE INDEX idx_data_archives_method_bigint
  ON data_archives(method_id, parent_pk_bigint)
  WHERE parent_pk_bigint IS NOT NULL;

CREATE UNIQUE INDEX idx_data_archives_method_text
  ON data_archives(method_id, parent_pk_text)
  WHERE parent_pk_text IS NOT NULL;

-- 検索用インデックス
CREATE INDEX idx_data_archives_org ON data_archives(organization_id);
CREATE INDEX idx_data_archives_status ON data_archives(status);
CREATE INDEX idx_data_archives_scheduled ON data_archives(scheduled_at) WHERE status = 'scheduled';

-- ステータス遷移ログ（監査用）
CREATE TABLE data_archive_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    archive_id UUID NOT NULL REFERENCES data_archives(id),
    action TEXT NOT NULL,                     -- 'archive', 'verify', 'delete_source', 'restore', 're_archive'
    old_status TEXT,
    new_status TEXT NOT NULL,
    details JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_data_archive_logs_archive ON data_archive_logs(archive_id);
```

### クエリ例

```sql
-- kudguriのアーカイブ状態を確認
SELECT k.uuid, k.unkou_no, da.status, da.scheduled_at, da.archived_at
FROM kudguri k
LEFT JOIN data_archives da
  ON da.method_id = 'kudguri' AND da.parent_pk_uuid = k.uuid;

-- 未登録のkudguriを取得（アーカイブ登録が必要）
SELECT k.*
FROM kudguri k
LEFT JOIN data_archives da
  ON da.method_id = 'kudguri' AND da.parent_pk_uuid = k.uuid
WHERE da.id IS NULL;

-- アーカイブ済みファイルの詳細
SELECT da.id, da.status, f->>'file' as file, f->>'checksum' as checksum, f->>'record_count' as records
FROM data_archives da, jsonb_array_elements(da.files) f
WHERE da.method_id = 'kudguri' AND da.parent_pk_uuid = 'xxx';
```

### ステータス遷移

```
scheduled → pending → archived → verified → deleted (source_deleted)
                                     ↓
                                 restored → scheduled (re-archive cycle)
```

| ステータス | 説明 |
|-----------|------|
| scheduled | アーカイブ予定（scheduled_at待ち） |
| pending | アーカイブ処理待ち（scheduled_at到達） |
| archived | GCSへの保存完了 |
| verified | checksum検証完了、削除可能 |
| deleted | 元データ削除済み |
| restored | GCSから復元済み（DBにデータあり） |

## 操作フロー

### データ取り込み時（scheduled登録）

```sql
-- 新規データ: 3ヶ月後にアーカイブ予定
INSERT INTO data_archives (
    organization_id, source_table, source_pk_uuid,
    parent_table, parent_pk_uuid,
    storage_path, checksum, byte_size, status, scheduled_at
) VALUES (
    'org-uuid', 'kudgivt', 'record-uuid',
    'kudguri', 'parent-uuid',
    'org-uuid/kudguri/parent-uuid/ivt.json', '', 0, 'scheduled',
    now() + INTERVAL '3 months'
);

-- 過去データ取り込み: 即アーカイブ対象
INSERT INTO data_archives (..., scheduled_at)
VALUES (..., now());
```

### Archive (DB → GCS)

```
1. scheduled → pending: バッチでscheduled_at到達分を処理開始
   ↓
2. pending → archived: DBからデータ取得 → JSON化 → GCSに保存
   ↓
3. archived → verified: GCSからダウンロード → checksum照合
   ↓
4. verified → deleted: 元テーブルからDELETE
```

```sql
-- 0. スケジュール到達分をpendingに
UPDATE data_archives
SET status = 'pending', updated_at = now()
WHERE status = 'scheduled' AND scheduled_at <= now();

-- 1. pending対象を取得
SELECT * FROM data_archives WHERE status = 'pending';

-- 2. GCS保存後
UPDATE data_archives
SET status = 'archived', archived_at = now(), updated_at = now()
WHERE id = '...';

-- 3. checksum検証後
UPDATE data_archives
SET status = 'verified', verified_at = now(), updated_at = now()
WHERE id = '...';

-- 4. 元データ削除
DELETE FROM kudgivt WHERE uuid = 'record-uuid';
UPDATE data_archives
SET status = 'deleted', source_deleted_at = now(), updated_at = now()
WHERE id = '...';
```

### Restore (GCS → DB)

```
1. deleted状態のレコードを選択
   ↓
2. GCSからJSONダウンロード → checksum検証
   ↓
3. DBにINSERT
   ↓
4. restored: restored_at更新、status = 'restored'
```

```sql
-- 1. 復元対象を取得
SELECT * FROM data_archives
WHERE status = 'deleted' AND source_table = 'kudgivt' AND source_pk_uuid = 'record-uuid';

-- 2-3. GCSからダウンロード → DBにINSERT
INSERT INTO kudgivt (...) VALUES (...);

-- 4. ステータス更新
UPDATE data_archives
SET status = 'restored', restored_at = now(), updated_at = now()
WHERE id = '...';
```

### Re-archive (復元データを再アーカイブ)

```
1. restored状態のレコードを選択
   ↓
2. DBから最新データ取得 → JSON化 → GCS上書き
   ↓
3. checksum/byte_size更新
   ↓
4. archived: restored_at = NULL、archived_at更新
   ↓
5. verified → deleted (通常フローに戻る)
```

```sql
-- 1. 再アーカイブ対象を取得
SELECT * FROM data_archives WHERE status = 'restored';

-- 2-4. DBから取得 → GCS上書き → ステータス更新
UPDATE data_archives
SET status = 'archived',
    archived_at = now(),
    restored_at = NULL,
    checksum = 'new-sha256...',
    byte_size = 5678,
    updated_at = now()
WHERE id = '...';

-- 5. 以降は通常のArchiveフローと同じ
-- verified → deleted
```

### 監査ログ記録

各操作時に`data_archive_logs`に記録:

```sql
INSERT INTO data_archive_logs (archive_id, action, old_status, new_status, details)
VALUES ('archive-id', 'archive', 'pending', 'archived', '{"gcs_upload_time_ms": 150}');

INSERT INTO data_archive_logs (archive_id, action, old_status, new_status, details)
VALUES ('archive-id', 'restore', 'deleted', 'restored', '{"reason": "user_request"}');

INSERT INTO data_archive_logs (archive_id, action, old_status, new_status, details)
VALUES ('archive-id', 're_archive', 'restored', 'archived', '{"data_changed": true}');
```

## 実装ステップ

### 1. マイグレーション
- `data_archives`テーブル作成
- `data_archive_logs`テーブル作成

### 2. Rustコード実装
- `src/storage/archive.rs` - 汎用アーカイブ操作
- `src/db/archive.rs` - 追跡テーブル操作

### 3. gRPCサービス更新
- kudgivt等の新規保存時はGCSに直接保存
- 取得時はGCSから読み取り

### 4. DBテーブル削除
- `verified`状態のレコードのソーステーブルデータを削除

## コスト削減効果

| 項目 | Cloud SQL | GCS Coldline |
|------|-----------|--------------|
| 単価 | $0.17/GB/月 | $0.004/GB/月 |
| 現在(2.8MB) | $0.00048/月 | $0.000011/月 |

※データ量が増えるほど効果大

## 参考

- [Data Archiving Best Practices - Cloudian](https://cloudian.com/guides/data-backup/data-archiving-strategy-in-2025-methods-and-best-practices/)
- [PostgreSQL Data Archiving - Data Egret](https://dataegret.com/2025/05/data-archiving-and-retention-in-postgresql-best-practices-for-large-datasets/)
