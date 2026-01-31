#!/usr/bin/env python3
"""
Incremental import: insert only NEW records from db202601301200.zip
compared to db202601031200.zip.

Tables: car_inspection, files, car_inspection_files_a/b, car_ins_sheet_ichiban_cars_a
Also uploads new file blobs to GCS.

Usage:
    # 1. Start Cloud SQL Proxy
    ./start-proxy.sh

    # 2. Run import
    python3 incremental_import.py
"""

import zipfile
import re
import base64
import psycopg2
from google.cloud import storage
from tqdm import tqdm

# Configuration
OLD_ZIP = "db202601031200.zip"
NEW_ZIP = "db202601301200.zip"
OLD_SQL = "db202601031200.sql"
NEW_SQL = "db202601301200.sql"

ORG_ID = "00000000-0000-0000-0000-000000000001"
GCS_BUCKET = "rust-logi-files"
DB_URL = "postgres://postgres:kikuraku@127.0.0.1:5432/rust_logi_test"


def extract_table(zip_path, sql_name, table_name):
    """Extract columns and rows for a table from a pg_dump zip."""
    with zipfile.ZipFile(zip_path) as z:
        with z.open(sql_name) as f:
            content = f.read().decode("utf-8", errors="replace")

    pattern = rf'COPY public\.{re.escape(table_name)} \(([^)]+)\) FROM stdin;\n(.*?)\n\\.'
    match = re.search(pattern, content, re.DOTALL)
    if not match:
        return [], []

    cols = [c.strip().strip('"') for c in match.group(1).split(",")]
    rows_raw = match.group(2).strip().split("\n")
    rows = []
    for r in rows_raw:
        fields = r.split("\t")
        if len(fields) == len(cols):
            rows.append(fields)
    return cols, rows


def find_new_rows(old_rows, new_rows, key_idx):
    """Find rows in new_rows whose key is not in old_rows."""
    old_keys = set(r[key_idx] for r in old_rows)
    return [r for r in new_rows if r[key_idx] not in old_keys]


def find_new_rows_multi_key(old_rows, new_rows, key_indices):
    """Find rows in new_rows whose composite key is not in old_rows."""
    old_keys = set(tuple(r[i] for i in key_indices) for r in old_rows)
    return [r for r in new_rows if tuple(r[i] for i in key_indices) not in old_keys]


def fix_grantdate_space(value):
    """Add space prefix to single-digit Grantdate values."""
    stripped = value.strip()
    if stripped.isdigit() and int(stripped) < 10 and not value.startswith(" "):
        return " " + stripped
    return value


def escape_sql_value(value, col_name=None):
    """Escape a value for SQL INSERT."""
    if value == "\\N":
        return "NULL"

    if col_name in ("created_at", "modified_at", "deleted_at"):
        if value == "":
            return "NULL"
        if value.isdigit() and len(value) >= 13:
            return f"to_timestamp({value}::bigint / 1000.0)"
        if "T" in value and len(value) > 10:
            return f"'{value.replace(chr(39), chr(39)*2)}'::timestamptz"
        return "NULL"

    if value == "":
        return "''"

    return f"'{value.replace(chr(39), chr(39)*2)}'"


def run_sql(conn, sql):
    """Execute SQL and commit."""
    cur = conn.cursor()
    cur.execute(sql)
    conn.commit()
    return cur


def import_car_inspection(conn, new_rows, cols):
    """Import new car_inspection rows with Grantdate space fix."""
    if not new_rows:
        print("  car_inspection: no new rows")
        return

    # Column mapping
    col_mapping = {
        "created": "created_at",
        "Modified": "modified_at",
        "modified": "modified_at",
        "deleted": "deleted_at",
    }

    # Grantdate columns that need space fix
    grantdate_cols = {
        "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD",
        "ElectCertPublishdateE", "ElectCertPublishdateY", "ElectCertPublishdateM", "ElectCertPublishdateD",
        "ReggrantdateE", "ReggrantdateY", "ReggrantdateM", "ReggrantdateD",
        "FirstregistdateE", "FirstregistdateY", "FirstregistdateM",
        "ValidPeriodExpirdateE", "ValidPeriodExpirdateY", "ValidPeriodExpirdateM", "ValidPeriodExpirdateD",
    }

    # Build column list: original[0], organization_id, original[1:]
    mapped_cols = []
    for c in cols:
        mapped_cols.append(col_mapping.get(c, c))

    # Insert with org_id at position 1 (after first column)
    insert_cols = [mapped_cols[0], "organization_id"] + mapped_cols[1:]

    values_list = []
    for row in new_rows:
        escaped = [escape_sql_value(row[0], mapped_cols[0])]
        escaped.append(f"'{ORG_ID}'")
        for i in range(1, len(row)):
            value = row[i]
            orig_col = cols[i]
            if orig_col in grantdate_cols:
                value = fix_grantdate_space(value)
            escaped.append(escape_sql_value(value, mapped_cols[i]))
        values_list.append(f"({', '.join(escaped)})")

    col_names = ", ".join([f'"{c}"' for c in insert_cols])
    sql = f"INSERT INTO car_inspection ({col_names}) VALUES {', '.join(values_list)} ON CONFLICT DO NOTHING;"

    cur = conn.cursor()
    cur.execute(sql)
    conn.commit()
    print(f"  car_inspection: inserted {cur.rowcount} / {len(new_rows)} rows")


def import_files_metadata(conn, new_rows, cols):
    """Import new files rows (metadata only, blob=NULL, s3_key set)."""
    if not new_rows:
        print("  files: no new rows")
        return

    uuid_idx = cols.index("uuid")
    filename_idx = cols.index("filename")
    created_idx = cols.index("created")
    deleted_idx = cols.index("deleted")
    type_idx = cols.index("type")

    for row in tqdm(new_rows, desc="  files"):
        uuid = row[uuid_idx]
        filename = row[filename_idx].replace("'", "''")
        file_type = row[type_idx].replace("'", "''")
        s3_key = f"{ORG_ID}/{uuid}"

        created_val = escape_sql_value(row[created_idx], "created_at")
        deleted_val = escape_sql_value(row[deleted_idx], "deleted_at")

        sql = f"""
        INSERT INTO files (uuid, organization_id, filename, type, blob, s3_key, storage_class, created_at, deleted_at)
        VALUES ('{uuid}', '{ORG_ID}', '{filename}', '{file_type}', NULL, '{s3_key}', 'STANDARD', {created_val}, {deleted_val})
        ON CONFLICT DO NOTHING;
        """
        cur = conn.cursor()
        cur.execute(sql)
        conn.commit()

    print(f"  files: processed {len(new_rows)} rows")


def import_car_inspection_files(conn, new_rows, cols, table_name):
    """Import new car_inspection_files_a or _b rows with Grantdate space fix."""
    if not new_rows:
        print(f"  {table_name}: no new rows")
        return

    col_mapping = {
        "created": "created_at",
        "modified": "modified_at",
        "deleted": "deleted_at",
    }

    grantdate_cols = {"GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD"}

    mapped_cols = [col_mapping.get(c, c) for c in cols]
    insert_cols = [mapped_cols[0], "organization_id"] + mapped_cols[1:]

    values_list = []
    for row in new_rows:
        escaped = [escape_sql_value(row[0], mapped_cols[0])]
        escaped.append(f"'{ORG_ID}'")
        for i in range(1, len(row)):
            value = row[i]
            orig_col = cols[i]
            if orig_col in grantdate_cols:
                value = fix_grantdate_space(value)
            escaped.append(escape_sql_value(value, mapped_cols[i]))
        values_list.append(f"({', '.join(escaped)})")

    col_names = ", ".join([f'"{c}"' for c in insert_cols])
    sql = f"INSERT INTO {table_name} ({col_names}) VALUES {', '.join(values_list)} ON CONFLICT DO NOTHING;"

    cur = conn.cursor()
    cur.execute(sql)
    conn.commit()
    print(f"  {table_name}: inserted {cur.rowcount} / {len(new_rows)} rows")


def import_car_ins_sheet(conn, new_rows, cols):
    """Import new car_ins_sheet_ichiban_cars_a rows."""
    if not new_rows:
        print("  car_ins_sheet_ichiban_cars_a: no new rows")
        return

    # Add organization_id after first column
    insert_cols = [cols[0], "organization_id"] + cols[1:]

    values_list = []
    for row in new_rows:
        escaped = [escape_sql_value(row[0])]
        escaped.append(f"'{ORG_ID}'")
        for i in range(1, len(row)):
            escaped.append(escape_sql_value(row[i]))
        values_list.append(f"({', '.join(escaped)})")

    col_names = ", ".join([f'"{c}"' for c in insert_cols])
    sql = f"INSERT INTO car_ins_sheet_ichiban_cars_a ({col_names}) VALUES {', '.join(values_list)} ON CONFLICT DO NOTHING;"

    cur = conn.cursor()
    cur.execute(sql)
    conn.commit()
    print(f"  car_ins_sheet_ichiban_cars_a: inserted {cur.rowcount} / {len(new_rows)} rows")


def upload_to_gcs(new_file_rows, cols):
    """Upload new files to GCS from dump blob data."""
    if not new_file_rows:
        print("\nGCS: no files to upload")
        return

    uuid_idx = cols.index("uuid")
    blob_idx = cols.index("blob")
    type_idx = cols.index("type")
    filename_idx = cols.index("filename")

    print(f"\nUploading {len(new_file_rows)} files to GCS bucket '{GCS_BUCKET}'...")
    gcs_client = storage.Client()
    bucket = gcs_client.bucket(GCS_BUCKET)

    uploaded = 0
    errors = 0
    for row in tqdm(new_file_rows, desc="  GCS upload"):
        uuid = row[uuid_idx]
        blob_data = row[blob_idx]
        content_type = row[type_idx]
        filename = row[filename_idx]

        if blob_data == "\\N" or not blob_data:
            print(f"    Skip {uuid}: no blob data")
            continue

        try:
            data = base64.b64decode(blob_data)
            gcs_key = f"{ORG_ID}/{uuid}"
            blob_obj = bucket.blob(gcs_key)
            blob_obj.upload_from_string(data, content_type=content_type)
            uploaded += 1
        except Exception as e:
            print(f"    Error uploading {uuid} ({filename}): {e}")
            errors += 1

    print(f"  GCS: uploaded {uploaded}, errors {errors}")


def main():
    print("=== Incremental Import ===")
    print(f"Old: {OLD_ZIP}")
    print(f"New: {NEW_ZIP}")
    print()

    # 1. Extract and compare
    print("Extracting and comparing dumps...")

    # car_inspection
    old_ci_cols, old_ci_rows = extract_table(OLD_ZIP, OLD_SQL, "car_inspection")
    new_ci_cols, new_ci_rows = extract_table(NEW_ZIP, NEW_SQL, "car_inspection")
    ci_key_idx = new_ci_cols.index("ElectCertMgNo")
    ci_gy_idx = new_ci_cols.index("GrantdateY")
    # Use composite key: (ElectCertMgNo, GrantdateE, GrantdateY, GrantdateM, GrantdateD)
    ci_key_indices = [
        new_ci_cols.index("ElectCertMgNo"),
        new_ci_cols.index("GrantdateE"),
        new_ci_cols.index("GrantdateY"),
        new_ci_cols.index("GrantdateM"),
        new_ci_cols.index("GrantdateD"),
    ]
    new_ci = find_new_rows_multi_key(old_ci_rows, new_ci_rows, ci_key_indices)

    # files
    old_f_cols, old_f_rows = extract_table(OLD_ZIP, OLD_SQL, "files")
    new_f_cols, new_f_rows = extract_table(NEW_ZIP, NEW_SQL, "files")
    f_uuid_idx = new_f_cols.index("uuid")
    new_files = find_new_rows(old_f_rows, new_f_rows, f_uuid_idx)

    # car_inspection_files_a
    old_fa_cols, old_fa_rows = extract_table(OLD_ZIP, OLD_SQL, "car_inspection_files_a")
    new_fa_cols, new_fa_rows = extract_table(NEW_ZIP, NEW_SQL, "car_inspection_files_a")
    fa_uuid_idx = new_fa_cols.index("uuid")
    new_fa = find_new_rows(old_fa_rows, new_fa_rows, fa_uuid_idx)

    # car_inspection_files_b
    old_fb_cols, old_fb_rows = extract_table(OLD_ZIP, OLD_SQL, "car_inspection_files_b")
    new_fb_cols, new_fb_rows = extract_table(NEW_ZIP, NEW_SQL, "car_inspection_files_b")
    fb_uuid_idx = new_fb_cols.index("uuid")
    new_fb = find_new_rows(old_fb_rows, new_fb_rows, fb_uuid_idx)

    # car_ins_sheet_ichiban_cars_a
    old_cs_cols, old_cs_rows = extract_table(OLD_ZIP, OLD_SQL, "car_ins_sheet_ichiban_cars_a")
    new_cs_cols, new_cs_rows = extract_table(NEW_ZIP, NEW_SQL, "car_ins_sheet_ichiban_cars_a")
    # Composite key: (ElectCertMgNo, GrantdateE, GrantdateY, GrantdateM, GrantdateD)
    cs_key_indices = [
        new_cs_cols.index("ElectCertMgNo"),
        new_cs_cols.index("GrantdateE"),
        new_cs_cols.index("GrantdateY"),
        new_cs_cols.index("GrantdateM"),
        new_cs_cols.index("GrantdateD"),
    ]
    new_cs = find_new_rows_multi_key(old_cs_rows, new_cs_rows, cs_key_indices)

    print(f"\nNew records to import:")
    print(f"  car_inspection:                {len(new_ci)}")
    print(f"  files:                         {len(new_files)}")
    print(f"  car_inspection_files_a:        {len(new_fa)}")
    print(f"  car_inspection_files_b:        {len(new_fb)}")
    print(f"  car_ins_sheet_ichiban_cars_a:  {len(new_cs)}")

    if not any([new_ci, new_files, new_fa, new_fb, new_cs]):
        print("\nNothing to import!")
        return

    # 2. Connect to DB
    print(f"\nConnecting to database...")
    conn = psycopg2.connect(DB_URL)

    # Set RLS organization context
    cur = conn.cursor()
    cur.execute(f"SET app.current_organization_id = '{ORG_ID}';")
    conn.commit()
    print(f"  RLS organization set to {ORG_ID}")

    # 3. Import to DB
    print("\nImporting to database...")
    import_files_metadata(conn, new_files, new_f_cols)
    import_car_inspection(conn, new_ci, new_ci_cols)
    import_car_inspection_files(conn, new_fa, new_fa_cols, "car_inspection_files_a")
    import_car_inspection_files(conn, new_fb, new_fb_cols, "car_inspection_files_b")
    import_car_ins_sheet(conn, new_cs, new_cs_cols)

    conn.close()

    # 4. Upload to GCS
    upload_to_gcs(new_files, new_f_cols)

    print("\n=== Done ===")


if __name__ == "__main__":
    main()
