#!/usr/bin/env -S cargo +nightly -Zscript
//! 既存のDBに保存されたファイルをS3に移行するスクリプト
//!
//! 使用方法:
//!   1. 環境変数を設定:
//!      export DATABASE_URL="postgres://..."
//!      export S3_BUCKET="rust-logi-files"
//!      export AWS_REGION="ap-northeast-1"
//!
//!   2. スクリプトを実行:
//!      cargo run --bin migrate-to-s3
//!
//! または別の方法:
//!   psqlで直接データを抽出してPythonスクリプトで移行

use std::env;

// このスクリプトは別のバイナリとして実行するか、
// 以下のSQLとスクリプトを手動で実行してください

/*
=== 移行手順 ===

1. 移行対象のファイルを確認:
   SELECT uuid, filename, type, length(blob) as blob_size
   FROM files
   WHERE blob IS NOT NULL AND s3_key IS NULL AND deleted_at IS NULL
   ORDER BY created_at;

2. バッチ処理用のスクリプト（Python例）:

```python
#!/usr/bin/env python3
import psycopg2
import boto3
import base64
from tqdm import tqdm

# 設定
DB_URL = "postgres://postgres:kikuraku@127.0.0.1:5432/rust_logi_test"
S3_BUCKET = "rust-logi-files"
BATCH_SIZE = 100

# 接続
conn = psycopg2.connect(DB_URL)
s3 = boto3.client('s3')

# 移行対象取得
cur = conn.cursor()
cur.execute("""
    SELECT uuid, organization_id, filename, type, blob
    FROM files
    WHERE blob IS NOT NULL AND s3_key IS NULL AND deleted_at IS NULL
    ORDER BY created_at
    LIMIT %s
""", (BATCH_SIZE,))

files = cur.fetchall()
print(f"Found {len(files)} files to migrate")

for uuid, org_id, filename, content_type, blob in tqdm(files):
    # S3キー生成
    s3_key = f"{org_id}/{uuid}"

    # Base64デコード
    data = base64.b64decode(blob)

    # S3にアップロード
    s3.put_object(
        Bucket=S3_BUCKET,
        Key=s3_key,
        Body=data,
        ContentType=content_type
    )

    # DB更新
    cur.execute("""
        UPDATE files
        SET s3_key = %s,
            storage_class = 'STANDARD',
            blob = NULL,
            last_accessed_at = NOW()
        WHERE uuid = %s
    """, (s3_key, uuid))

    conn.commit()

print("Migration complete!")
conn.close()
```

3. 移行結果を確認:
   SELECT
     COUNT(*) FILTER (WHERE s3_key IS NOT NULL) as migrated,
     COUNT(*) FILTER (WHERE blob IS NOT NULL) as remaining
   FROM files
   WHERE deleted_at IS NULL;

4. 移行完了後、blobカラムを削除（オプション）:
   -- 十分なテスト後に実行
   -- ALTER TABLE files DROP COLUMN blob;
*/

fn main() {
    println!("=== S3 Migration Script ===");
    println!();
    println!("This script provides SQL queries and Python code for migrating files from DB to S3.");
    println!();
    println!("Please see the comments in this file for detailed instructions.");
    println!();

    // 環境変数チェック
    let database_url = env::var("DATABASE_URL").ok();
    let s3_bucket = env::var("S3_BUCKET").ok();
    let aws_region = env::var("AWS_REGION").ok();

    println!("Environment variables:");
    println!("  DATABASE_URL: {}", database_url.as_deref().unwrap_or("NOT SET"));
    println!("  S3_BUCKET: {}", s3_bucket.as_deref().unwrap_or("NOT SET"));
    println!("  AWS_REGION: {}", aws_region.as_deref().unwrap_or("NOT SET"));
    println!();

    if database_url.is_none() || s3_bucket.is_none() {
        println!("Please set all required environment variables before running migration.");
        std::process::exit(1);
    }

    println!("To perform the migration, use the Python script in the comments above,");
    println!("or run the SQL queries directly.");
}
