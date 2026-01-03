#!/usr/bin/env python3
"""
Convert pg_dump SQL to multi-tenant format and import to rust_logi_test.

Usage:
    python3 convert_and_import.py
"""

import zipfile
import subprocess
import re
import os

# Configuration
ZIP_FILE = "db202601031200.zip"
SQL_FILE = "db202601031200.sql"

# Test organization ID
TEST_ORG_ID = "00000000-0000-0000-0000-000000000001"
TEST_ORG_NAME = "Test Organization"

# Tables that need organization_id added (in import order - parents first)
IMPORT_ORDER = [
    "files",
    "car_inspection",
    "car_inspection_files",
    "car_inspection_deregistration",
    "car_inspection_deregistration_files",
    "ichiban_cars",
    "kudguri",  # Parent table - must be first
    "kudgcst",
    "kudgfry",
    "kudgful",
    "kudgivt",
    "kudgsir",
]

# Tables to skip entirely
SKIP_TABLES = {
    "drizzle.__drizzle_migrations",
    "my_schema.users",
    "public.users",
    "public.uriage",
    "public.uriage_jisha",
    "public.cam_files",
    "public.cam_file_exe",
    "public.cam_file_exe_stage",
    "public.flickr_photo",
    "public.dtako_cars_ichiban_cars",
    "public.car_ins_sheet_ichiban_cars",
    "public.car_ins_sheet_ichiban_cars_a",
    "public.car_inspection_files_a",
    "public.car_inspection_files_b",
}


def extract_sql():
    """Extract SQL file from zip."""
    print(f"Extracting {SQL_FILE} from {ZIP_FILE}...")
    with zipfile.ZipFile(ZIP_FILE) as z:
        z.extractall(".")
    print("Done.")


def create_test_org():
    """Create test organization if not exists."""
    print(f"Creating test organization {TEST_ORG_ID}...")
    sql = f"""
    INSERT INTO organizations (id, name, slug, created_at, updated_at)
    VALUES ('{TEST_ORG_ID}', '{TEST_ORG_NAME}', 'test-org', NOW(), NOW())
    ON CONFLICT (id) DO NOTHING;
    """
    run_psql(sql)
    print("Done.")


def delete_existing_data():
    """Delete existing test data."""
    print("Deleting existing test data...")
    # Reverse order for FK constraints
    tables = list(reversed(IMPORT_ORDER))
    for table in tables:
        sql = f"DELETE FROM {table} WHERE organization_id = '{TEST_ORG_ID}';"
        run_psql(sql, check=False)
    print("Done.")


def run_psql(sql, check=True):
    """Run SQL via psql."""
    env = os.environ.copy()
    env["PGPASSWORD"] = "kikuraku"
    result = subprocess.run(
        ["psql", "-h", "127.0.0.1", "-p", "5432", "-U", "postgres", "-d", "rust_logi_test", "-c", sql],
        env=env,
        capture_output=True,
        text=True,
        check=False
    )
    if result.returncode != 0 and check:
        print(f"Error: {result.stderr}")
    return result


def run_psql_from_file(filepath):
    """Run SQL from file via psql."""
    env = os.environ.copy()
    env["PGPASSWORD"] = "kikuraku"
    result = subprocess.run(
        ["psql", "-h", "127.0.0.1", "-p", "5432", "-U", "postgres", "-d", "rust_logi_test", "-f", filepath],
        env=env,
        capture_output=True,
        text=True
    )
    return result


def get_column_mapping(table):
    """Get column name mapping for a table."""
    # Common mappings for kudg* tables
    kudg_common = {
        "unkouNo": "unkou_no",
        "unkouDate": "unkou_date",
        "kudguriUuid": "kudguri_uuid",
        "readDate": "read_date",
        "officeCd": "office_cd",
        "officeName": "office_name",
        "vehicleCd": "vehicle_cd",
        "vehicleName": "vehicle_name",
        "driverCd1": "driver_cd1",
        "driverName1": "driver_name1",
        "driverCd2": "driver_cd2",
        "driverName2": "driver_name2",
        "targetDriverType": "target_driver_type",
        "targetDriverCd": "target_driver_cd",
        "targetDriverName": "target_driver_name",
        "startDatetime": "start_datetime",
        "endDatetime": "end_datetime",
        "eventCd": "event_cd",
        "eventName": "event_name",
        "startMileage": "start_mileage",
        "endMileage": "end_mileage",
        "sectionTime": "section_time",
        "sectionDistance": "section_distance",
        "startCityCd": "start_city_cd",
        "startCityName": "start_city_name",
        "endCityCd": "end_city_cd",
        "endCityName": "end_city_name",
        "startPlaceCd": "start_place_cd",
        "startPlaceName": "start_place_name",
        "endPlaceCd": "end_place_cd",
        "endPlaceName": "end_place_name",
        "startGpsValid": "start_gps_valid",
        "startGpsLat": "start_gps_lat",
        "startGpsLng": "start_gps_lng",
        "endGpsValid": "end_gps_valid",
        "endGpsLat": "end_gps_lat",
        "endGpsLng": "end_gps_lng",
        "overLimitMax": "over_limit_max",
        "created": "created_at",
        "deleted": "deleted_at",
        # kudgcst specific
        "ferryCompanyCd": "ferry_company_cd",
        "ferryCompanyName": "ferry_company_name",
        "boardingPlaceCd": "boarding_place_cd",
        "boardingPlaceName": "boarding_place_name",
        "tripNumber": "trip_number",
        "dropoffPlaceCd": "dropoff_place_cd",
        "dropoffPlaceName": "dropoff_place_name",
        "settlementType": "settlement_type",
        "settlementTypeName": "settlement_type_name",
        "standardFare": "standard_fare",
        "contractFare": "contract_fare",
        "ferryVehicleType": "ferry_vehicle_type",
        "ferryVehicleTypeName": "ferry_vehicle_type_name",
        "assumedDistance": "assumed_distance",
        # kudgfry specific
        "relevantDatetime": "relevant_datetime",
        "refuelInspectCategory": "refuel_inspect_category",
        "refuelInspectCategoryName": "refuel_inspect_category_name",
        "refuelInspectType": "refuel_inspect_type",
        "refuelInspectTypeName": "refuel_inspect_type_name",
        "refuelInspectKind": "refuel_inspect_kind",
        "refuelInspectKindName": "refuel_inspect_kind_name",
        "refillAmount": "refill_amount",
        "ownOtherType": "own_other_type",
        "meterValue": "meter_value",
        # kudgivt specific
        "clockInDatetime": "clock_in_datetime",
        "clockOutDatetime": "clock_out_datetime",
        "departureDatetime": "departure_datetime",
        "returnDatetime": "return_datetime",
        "departureMeter": "departure_meter",
        "returnMeter": "return_meter",
        "totalMileage": "total_mileage",
        "destinationCityName": "destination_city_name",
        "destinationPlaceName": "destination_place_name",
        "actualMileage": "actual_mileage",
        "localDriveTime": "local_drive_time",
        "expressDriveTime": "express_drive_time",
        "bypassDriveTime": "bypass_drive_time",
        "actualDriveTime": "actual_drive_time",
        "emptyDriveTime": "empty_drive_time",
        "work1Time": "work1_time",
        "work2Time": "work2_time",
        "work3Time": "work3_time",
        "work4Time": "work4_time",
        "work5Time": "work5_time",
        "work6Time": "work6_time",
        "work7Time": "work7_time",
        "work8Time": "work8_time",
        "work9Time": "work9_time",
        "work10Time": "work10_time",
        "state1Distance": "state1_distance",
        "state1Time": "state1_time",
        "state2Distance": "state2_distance",
        "state2Time": "state2_time",
        "state3Distance": "state3_distance",
        "state3Time": "state3_time",
        "state4Distance": "state4_distance",
        "state4Time": "state4_time",
        "state5Distance": "state5_distance",
        "state5Time": "state5_time",
        "ownMainFuel": "own_main_fuel",
        "ownMainAdditive": "own_main_additive",
        "ownConsumable": "own_consumable",
        "otherMainFuel": "other_main_fuel",
        "otherMainAdditive": "other_main_additive",
        "otherConsumable": "other_consumable",
        "localSpeedOverMax": "local_speed_over_max",
        "localSpeedOverTime": "local_speed_over_time",
        "localSpeedOverCount": "local_speed_over_count",
        "expressSpeedOverMax": "express_speed_over_max",
        "expressSpeedOverTime": "express_speed_over_time",
        "expressSpeedOverCount": "express_speed_over_count",
        "dedicatedSpeedOverMax": "dedicated_speed_over_max",
        "dedicatedSpeedOverTime": "dedicated_speed_over_time",
        "dedicatedSpeedOverCount": "dedicated_speed_over_count",
        "idlingTime": "idling_time",
        "idlingTimeCount": "idling_time_count",
        "rotationOverMax": "rotation_over_max",
        "rotationOverCount": "rotation_over_count",
        "rotationOverTime": "rotation_over_time",
        "rapidAccelCount1": "rapid_accel_count1",
        "rapidAccelCount2": "rapid_accel_count2",
        "rapidAccelCount3": "rapid_accel_count3",
        "rapidAccelCount4": "rapid_accel_count4",
        "rapidAccelCount5": "rapid_accel_count5",
        "rapidAccelMax": "rapid_accel_max",
        "rapidAccelMaxSpeed": "rapid_accel_max_speed",
        "rapidDecelCount1": "rapid_decel_count1",
        "rapidDecelCount2": "rapid_decel_count2",
        "rapidDecelCount3": "rapid_decel_count3",
        "rapidDecelCount4": "rapid_decel_count4",
        "rapidDecelCount5": "rapid_decel_count5",
        "rapidDecelMax": "rapid_decel_max",
        "rapidDecelMaxSpeed": "rapid_decel_max_speed",
        "rapidCurveCount1": "rapid_curve_count1",
        "rapidCurveCount2": "rapid_curve_count2",
        "rapidCurveCount3": "rapid_curve_count3",
        "rapidCurveCount4": "rapid_curve_count4",
        "rapidCurveCount5": "rapid_curve_count5",
        "rapidCurveMax": "rapid_curve_max",
        "rapidCurveMaxSpeed": "rapid_curve_max_speed",
        "continuousDriveOverCount": "continuous_drive_over_count",
        "continuousDriveMaxTime": "continuous_drive_max_time",
        "continuousDriveTotalTime": "continuous_drive_total_time",
        "waveDriveCount": "wave_drive_count",
        "waveDriveMaxTime": "wave_drive_max_time",
        "waveDriveMaxSpeedDiff": "wave_drive_max_speed_diff",
        "localSpeedScore": "local_speed_score",
        "expressSpeedScore": "express_speed_score",
        "dedicatedSpeedScore": "dedicated_speed_score",
        "localDistanceScore": "local_distance_score",
        "expressDistanceScore": "express_distance_score",
        "dedicatedDistanceScore": "dedicated_distance_score",
        "rapidAccelScore": "rapid_accel_score",
        "rapidDecelScore": "rapid_decel_score",
        "rapidCurveScore": "rapid_curve_score",
        "actualLowSpeedRotationScore": "actual_low_speed_rotation_score",
        "actualHighSpeedRotationScore": "actual_high_speed_rotation_score",
        "emptyLowSpeedRotationScore": "empty_low_speed_rotation_score",
        "emptyHighSpeedRotationScore": "empty_high_speed_rotation_score",
        "idlingScore": "idling_score",
        "continuousDriveScore": "continuous_drive_score",
        "waveDriveScore": "wave_drive_score",
        "safetyScore": "safety_score",
        "economyScore": "economy_score",
        "totalScore": "total_score",
    }

    if table.startswith("kudg"):
        return kudg_common

    if table == "files":
        return {
            "created": "created_at",
            "deleted": "deleted_at",
        }

    if table == "ichiban_cars":
        return {
            "name_R": "name_r",
        }

    if table.startswith("car_inspection"):
        return {
            "created": "created_at",
            "Modified": "modified_at",
            "modified": "modified_at",
            "deleted": "deleted_at",
            "fileUuid": "file_uuid",
        }

    return {}


def convert_and_import():
    """Convert SQL and import data."""
    print(f"Reading {SQL_FILE}...")
    with open(SQL_FILE, "r") as f:
        content = f.read()

    lines = content.split("\n")
    tables_data = {}
    in_copy = False
    current_table = None
    skip_data = False
    copy_columns = []

    i = 0
    while i < len(lines):
        line = lines[i]

        # Handle COPY statements
        if line.startswith("COPY "):
            match = re.match(r"COPY ([\w.]+) \((.*)\) FROM stdin;", line)
            if match:
                table_name = match.group(1)
                columns = match.group(2)

                # Normalize table name
                if table_name.startswith("public."):
                    short_name = table_name[7:]
                else:
                    short_name = table_name

                if table_name in SKIP_TABLES:
                    skip_data = True
                    in_copy = True
                    current_table = None
                    i += 1
                    continue

                if short_name not in IMPORT_ORDER:
                    skip_data = True
                    in_copy = True
                    current_table = None
                    i += 1
                    continue

                in_copy = True
                current_table = short_name
                skip_data = False
                copy_columns = [c.strip().strip('"') for c in columns.split(",")]
                print(f"Processing table: {short_name} ({len(copy_columns)} columns)")

                if current_table not in tables_data:
                    tables_data[current_table] = {"cols": copy_columns, "rows": []}

            i += 1
            continue

        # End of COPY data
        if line == "\\." and in_copy:
            in_copy = False
            current_table = None
            skip_data = False
            i += 1
            continue

        # Process data rows
        if in_copy:
            if skip_data:
                i += 1
                continue

            if current_table:
                values = line.split("\t")
                if len(values) == len(copy_columns):
                    # Insert org_id at position 1 (after uuid/id)
                    values.insert(1, TEST_ORG_ID)
                    tables_data[current_table]["rows"].append(values)
            i += 1
            continue

        i += 1

    # Import in correct order
    print("\nImporting data...")
    for table in IMPORT_ORDER:
        if table in tables_data:
            import_table(table, tables_data[table]["cols"], tables_data[table]["rows"])
        else:
            print(f"  {table}: no data found")


def import_table(table, original_cols, rows):
    """Import data for a single table using INSERT."""
    if not rows:
        print(f"  {table}: no data")
        return

    # Build column list with organization_id
    cols_with_org = [original_cols[0], "organization_id"] + original_cols[1:]

    # Map old column names to new snake_case names
    col_mapping = get_column_mapping(table)

    mapped_cols = []
    for c in cols_with_org:
        if c in col_mapping:
            mapped_cols.append(col_mapping[c])
        else:
            mapped_cols.append(c)

    print(f"  {table}: {len(rows)} rows...")

    batch_size = 50
    error_shown = False
    success_count = 0

    for batch_start in range(0, len(rows), batch_size):
        batch = rows[batch_start:batch_start + batch_size]
        values_list = []

        for row in batch:
            escaped = []
            for idx, v in enumerate(row):
                col_name = mapped_cols[idx] if idx < len(mapped_cols) else f"col{idx}"

                if v == "\\N":
                    escaped.append("NULL")
                elif col_name in ("created_at", "modified_at", "deleted_at"):
                    # Handle timestamp columns
                    if v == "":
                        escaped.append("NULL")
                    elif v.isdigit() and len(v) >= 13:
                        # Unix milliseconds timestamp
                        escaped.append(f"to_timestamp({v}::bigint / 1000.0)")
                    elif "T" in v and len(v) > 10:
                        # ISO format timestamp
                        v_escaped = v.replace("'", "''")
                        escaped.append(f"'{v_escaped}'::timestamptz")
                    else:
                        escaped.append("NULL")
                elif v == "":
                    # Keep empty string for NOT NULL text columns
                    escaped.append("''")
                else:
                    # Escape single quotes
                    v_escaped = v.replace("'", "''")
                    escaped.append(f"'{v_escaped}'")
            values_list.append(f"({', '.join(escaped)})")

        # Quote column names properly
        col_names = ', '.join([f'"{c}"' for c in mapped_cols])
        sql = f"INSERT INTO {table} ({col_names}) VALUES {', '.join(values_list)} ON CONFLICT DO NOTHING;"

        # Write SQL to temp file and execute
        with open("/tmp/import_batch.sql", "w") as f:
            f.write(sql)

        result = run_psql_from_file("/tmp/import_batch.sql")
        if "ERROR" in result.stderr:
            if not error_shown:
                print(f"    Error: {result.stderr[:500]}")
                error_shown = True
        else:
            success_count += len(batch)

    if success_count > 0:
        print(f"    Imported {success_count} rows")


def main():
    extract_sql()
    create_test_org()
    delete_existing_data()
    convert_and_import()
    print("\nDone!")


if __name__ == "__main__":
    main()
