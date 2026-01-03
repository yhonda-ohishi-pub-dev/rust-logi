#!/usr/bin/env python3
"""
既存のDBに保存されたファイルをGCSに移行するスクリプト

使用方法:
  1. Cloud SQL Proxyを起動
  2. スクリプトを実行:
     python3 scripts/migrate_to_gcs.py

GCSバケットはAutoclassが有効なため、ストレージクラスは自動的に最適化されます。
"""

import psycopg2
from google.cloud import storage
import base64
from tqdm import tqdm
import sys

# 設定
DB_URL = "postgres://postgres:kikuraku@127.0.0.1:5432/rust_logi_test"
GCS_BUCKET = "rust-logi-files"
BATCH_SIZE = 100

def migrate():
    # 接続
    print(f"Connecting to database...")
    conn = psycopg2.connect(DB_URL)

    print(f"Connecting to GCS bucket: {GCS_BUCKET}")
    gcs_client = storage.Client()
    bucket = gcs_client.bucket(GCS_BUCKET)

    # 移行対象の総数を取得
    cur = conn.cursor()
    cur.execute("""
        SELECT COUNT(*)
        FROM files
        WHERE blob IS NOT NULL AND s3_key IS NULL AND deleted_at IS NULL
    """)
    total_count = cur.fetchone()[0]
    print(f"Found {total_count} files to migrate")

    if total_count == 0:
        print("No files to migrate!")
        return

    migrated = 0
    errors = 0

    while True:
        # バッチで移行対象取得
        cur.execute("""
            SELECT uuid, organization_id, filename, type, blob
            FROM files
            WHERE blob IS NOT NULL AND s3_key IS NULL AND deleted_at IS NULL
            ORDER BY created_at
            LIMIT %s
        """, (BATCH_SIZE,))

        files = cur.fetchall()
        if not files:
            break

        for uuid, org_id, filename, content_type, blob in tqdm(files, desc=f"Migrating batch"):
            try:
                # GCSキー生成
                gcs_key = f"{org_id}/{uuid}"

                # Base64デコード
                data = base64.b64decode(blob)

                # GCSにアップロード
                blob_obj = bucket.blob(gcs_key)
                blob_obj.upload_from_string(data, content_type=content_type)

                # DB更新（Autoclassが有効なのでstorage_classは設定しない）
                cur.execute("""
                    UPDATE files
                    SET s3_key = %s,
                        storage_class = 'STANDARD',
                        blob = NULL,
                        last_accessed_at = NOW()
                    WHERE uuid = %s
                """, (gcs_key, uuid))

                conn.commit()
                migrated += 1

            except Exception as e:
                print(f"\nError migrating {uuid}: {e}")
                conn.rollback()
                errors += 1

    print(f"\nMigration complete!")
    print(f"  Migrated: {migrated}")
    print(f"  Errors: {errors}")

    # 結果確認
    cur.execute("""
        SELECT
            COUNT(*) FILTER (WHERE s3_key IS NOT NULL) as migrated,
            COUNT(*) FILTER (WHERE blob IS NOT NULL) as remaining
        FROM files
        WHERE deleted_at IS NULL
    """)
    result = cur.fetchone()
    print(f"  Files in GCS: {result[0]}")
    print(f"  Files in DB (blob): {result[1]}")

    conn.close()

if __name__ == "__main__":
    migrate()
