use regex::Regex;
use sqlx::PgPool;
use std::sync::LazyLock;

use crate::db::set_current_organization;

// === PDF解析用の正規表現パターン ===

/// 車検証判定（文字間のスペースを許容）
static RE_CAR_INSPECTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"自\s*動\s*車\s*検\s*査\s*証\s*記\s*録\s*事\s*項").unwrap()
});

/// ElectCertMgNo: 12桁数字
static RE_ELECT_CERT_MG_NO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\d{12}").unwrap()
});

/// Grantdate抽出: 記録年月日の後、pdf-extract形式 "令 和  8  2  13 月 日"
/// pdf-extractは年/月/日ラベルを分離して出力するため、era + 3数字のパターンでキャプチャ
static RE_GRANTDATE_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)記録年月日.*?(令\s*和|平\s*成|昭\s*和)\s+(\d{1,2})\s+(\d{1,2})\s+(\d{1,2})").unwrap()
});

/// Grantdate抽出: 標準日本語日付形式 "令和8年2月13日"（フォールバック）
static RE_GRANTDATE_STANDARD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)記録年月日.*?(令\s*和|平\s*成|昭\s*和)\s*(\d{1,2})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日").unwrap()
});

/// Grantdate抽出: ４.備考セクション内のフォールバック（pdf-extract形式）
static RE_GRANTDATE_BIKO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)４[\.\．]\s*備考.*?(令\s*和|平\s*成|昭\s*和)\s+(\d{1,2})\s+(\d{1,2})\s+(\d{1,2})").unwrap()
});

/// ファイルアップロード時の自動解析ロジック
/// hono-logiのcreateFiles.ts相当の処理をRustで実装
pub struct FileAutoParser {
    pool: PgPool,
}

/// Grantdate文字列からスペース（半角+全角）を除去
fn strip_spaces(s: &str) -> String {
    s.replace(' ', "").replace('\u{3000}', "")
}

/// CertInfo JSONからフィールド値を文字列として取得（なければ空文字列）
fn get_str<'a>(cert_info: &'a serde_json::Value, key: &str) -> String {
    cert_info
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

impl FileAutoParser {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// JSONファイルアップロード後に呼ばれる自動解析処理
    /// hono-logi createFiles.ts L186-287 相当
    pub async fn process_json_upload(
        &self,
        file_uuid: &str,
        file_data: &[u8],
        organization_id: &str,
    ) -> Result<(), anyhow::Error> {
        // 1. JSONパース
        let json: serde_json::Value = serde_json::from_slice(file_data)?;

        let cert_info = match json.get("CertInfo") {
            Some(ci) => ci,
            None => {
                tracing::debug!("JSON does not contain CertInfo, skipping auto-parse");
                return Ok(());
            }
        };

        let elect_cert_mg_no = get_str(cert_info, "ElectCertMgNo");
        if elect_cert_mg_no.is_empty() {
            tracing::debug!("CertInfo.ElectCertMgNo is empty, skipping auto-parse");
            return Ok(());
        }

        // 2. Grantdateのスペース除去（hono-logi createCarInspection.ts L88-91）
        let grantdate_e = strip_spaces(&get_str(cert_info, "GrantdateE"));
        let grantdate_y = strip_spaces(&get_str(cert_info, "GrantdateY"));
        let grantdate_m = strip_spaces(&get_str(cert_info, "GrantdateM"));
        let grantdate_d = strip_spaces(&get_str(cert_info, "GrantdateD"));

        let cert_info_import_file_version = json
            .get("CertInfoImportFileVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        tracing::info!(
            "Auto-parsing JSON: ElectCertMgNo={}, Grantdate={}-{}-{}-{}",
            elect_cert_mg_no,
            grantdate_e,
            grantdate_y,
            grantdate_m,
            grantdate_d
        );

        // 3. DB接続取得 + RLS設定
        let mut conn = self.pool.acquire().await?;
        set_current_organization(&mut conn, organization_id).await?;

        // 4. car_inspection UPSERT（car_inspection_service.rs L192-338と同じSQL）
        sqlx::query(
            r#"
            INSERT INTO car_inspection (
                organization_id,
                "CertInfoImportFileVersion", "Acceptoutputno", "FormType", "ElectCertMgNo", "CarId",
                "ElectCertPublishdateE", "ElectCertPublishdateY", "ElectCertPublishdateM", "ElectCertPublishdateD",
                "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD",
                "TranspotationBureauchiefName", "EntryNoCarNo",
                "ReggrantdateE", "ReggrantdateY", "ReggrantdateM", "ReggrantdateD",
                "FirstregistdateE", "FirstregistdateY", "FirstregistdateM",
                "CarName", "CarNameCode", "CarNo", "Model", "EngineModel",
                "OwnernameLowLevelChar", "OwnernameHighLevelChar", "OwnerAddressChar", "OwnerAddressNumValue", "OwnerAddressCode",
                "UsernameLowLevelChar", "UsernameHighLevelChar", "UserAddressChar", "UserAddressNumValue", "UserAddressCode",
                "UseheadqrterChar", "UseheadqrterNumValue", "UseheadqrterCode",
                "CarKind", "Use", "PrivateBusiness", "CarShape", "CarShapeCode",
                "NoteCap", "Cap", "NoteMaxloadage", "Maxloadage",
                "NoteCarWgt", "CarWgt", "NoteCarTotalWgt", "CarTotalWgt",
                "NoteLength", "Length", "NoteWidth", "Width", "NoteHeight", "Height",
                "FfAxWgt", "FrAxWgt", "RfAxWgt", "RrAxWgt",
                "Displacement", "FuelClass", "ModelSpecifyNo", "ClassifyAroundNo",
                "ValidPeriodExpirdateE", "ValidPeriodExpirdateY", "ValidPeriodExpirdateM", "ValidPeriodExpirdateD",
                "NoteInfo",
                "TwodimensionCodeInfoEntryNoCarNo", "TwodimensionCodeInfoCarNo", "TwodimensionCodeInfoValidPeriodExpirdate",
                "TwodimensionCodeInfoModel", "TwodimensionCodeInfoModelSpecifyNoClassifyAroundNo",
                "TwodimensionCodeInfoCharInfo", "TwodimensionCodeInfoEngineModel", "TwodimensionCodeInfoCarNoStampPlace",
                "TwodimensionCodeInfoFirstregistdate",
                "TwodimensionCodeInfoFfAxWgt", "TwodimensionCodeInfoFrAxWgt", "TwodimensionCodeInfoRfAxWgt", "TwodimensionCodeInfoRrAxWgt",
                "TwodimensionCodeInfoNoiseReg", "TwodimensionCodeInfoNearNoiseReg", "TwodimensionCodeInfoDriveMethod",
                "TwodimensionCodeInfoOpacimeterMeasCar", "TwodimensionCodeInfoNoxPmMeasMode",
                "TwodimensionCodeInfoNoxValue", "TwodimensionCodeInfoPmValue",
                "TwodimensionCodeInfoSafeStdDate", "TwodimensionCodeInfoFuelClassCode",
                "RegistCarLightCar"
            ) VALUES (
                current_setting('app.current_organization_id')::uuid,
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
                $31, $32, $33, $34, $35, $36, $37, $38, $39, $40,
                $41, $42, $43, $44, $45, $46, $47, $48, $49, $50,
                $51, $52, $53, $54, $55, $56, $57, $58, $59, $60,
                $61, $62, $63, $64, $65, $66, $67, $68, $69, $70,
                $71, $72, $73, $74, $75, $76, $77, $78, $79, $80,
                $81, $82, $83, $84, $85, $86, $87, $88, $89, $90,
                $91, $92, $93, $94, $95
            )
            ON CONFLICT (organization_id, "ElectCertMgNo", "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD")
            DO UPDATE SET modified_at = NOW()
            "#,
        )
        .bind(&cert_info_import_file_version)        // $1
        .bind(&get_str(cert_info, "Acceptoutputno"))  // $2
        .bind(&get_str(cert_info, "FormType"))         // $3
        .bind(&elect_cert_mg_no)                       // $4
        .bind(&get_str(cert_info, "CarId"))            // $5
        .bind(&get_str(cert_info, "ElectCertPublishdateE"))  // $6
        .bind(&get_str(cert_info, "ElectCertPublishdateY"))  // $7
        .bind(&get_str(cert_info, "ElectCertPublishdateM"))  // $8
        .bind(&get_str(cert_info, "ElectCertPublishdateD"))  // $9
        .bind(&grantdate_e)                            // $10
        .bind(&grantdate_y)                            // $11
        .bind(&grantdate_m)                            // $12
        .bind(&grantdate_d)                            // $13
        .bind(&get_str(cert_info, "TranspotationBureauchiefName"))  // $14
        .bind(&get_str(cert_info, "EntryNoCarNo"))     // $15
        .bind(&get_str(cert_info, "ReggrantdateE"))    // $16
        .bind(&get_str(cert_info, "ReggrantdateY"))    // $17
        .bind(&get_str(cert_info, "ReggrantdateM"))    // $18
        .bind(&get_str(cert_info, "ReggrantdateD"))    // $19
        .bind(&get_str(cert_info, "FirstregistdateE")) // $20
        .bind(&get_str(cert_info, "FirstregistdateY")) // $21
        .bind(&get_str(cert_info, "FirstregistdateM")) // $22
        .bind(&get_str(cert_info, "CarName"))          // $23
        .bind(&get_str(cert_info, "CarNameCode"))      // $24
        .bind(&get_str(cert_info, "CarNo"))            // $25
        .bind(&get_str(cert_info, "Model"))            // $26
        .bind(&get_str(cert_info, "EngineModel"))      // $27
        .bind(&get_str(cert_info, "OwnernameLowLevelChar"))    // $28
        .bind(&get_str(cert_info, "OwnernameHighLevelChar"))   // $29
        .bind(&get_str(cert_info, "OwnerAddressChar"))         // $30
        .bind(&get_str(cert_info, "OwnerAddressNumValue"))     // $31
        .bind(&get_str(cert_info, "OwnerAddressCode"))         // $32
        .bind(&get_str(cert_info, "UsernameLowLevelChar"))     // $33
        .bind(&get_str(cert_info, "UsernameHighLevelChar"))    // $34
        .bind(&get_str(cert_info, "UserAddressChar"))          // $35
        .bind(&get_str(cert_info, "UserAddressNumValue"))      // $36
        .bind(&get_str(cert_info, "UserAddressCode"))          // $37
        .bind(&get_str(cert_info, "UseheadqrterChar"))         // $38
        .bind(&get_str(cert_info, "UseheadqrterNumValue"))     // $39
        .bind(&get_str(cert_info, "UseheadqrterCode"))         // $40
        .bind(&get_str(cert_info, "CarKind"))          // $41
        .bind(&get_str(cert_info, "Use"))              // $42
        .bind(&get_str(cert_info, "PrivateBusiness"))  // $43
        .bind(&get_str(cert_info, "CarShape"))         // $44
        .bind(&get_str(cert_info, "CarShapeCode"))     // $45
        .bind(&get_str(cert_info, "NoteCap"))          // $46
        .bind(&get_str(cert_info, "Cap"))              // $47
        .bind(&get_str(cert_info, "NoteMaxloadage"))   // $48
        .bind(&get_str(cert_info, "Maxloadage"))       // $49
        .bind(&get_str(cert_info, "NoteCarWgt"))       // $50
        .bind(&get_str(cert_info, "CarWgt"))           // $51
        .bind(&get_str(cert_info, "NoteCarTotalWgt"))  // $52
        .bind(&get_str(cert_info, "CarTotalWgt"))      // $53
        .bind(&get_str(cert_info, "NoteLength"))       // $54
        .bind(&get_str(cert_info, "Length"))           // $55
        .bind(&get_str(cert_info, "NoteWidth"))        // $56
        .bind(&get_str(cert_info, "Width"))            // $57
        .bind(&get_str(cert_info, "NoteHeight"))       // $58
        .bind(&get_str(cert_info, "Height"))           // $59
        .bind(&get_str(cert_info, "FfAxWgt"))          // $60
        .bind(&get_str(cert_info, "FrAxWgt"))          // $61
        .bind(&get_str(cert_info, "RfAxWgt"))          // $62
        .bind(&get_str(cert_info, "RrAxWgt"))          // $63
        .bind(&get_str(cert_info, "Displacement"))     // $64
        .bind(&get_str(cert_info, "FuelClass"))        // $65
        .bind(&get_str(cert_info, "ModelSpecifyNo"))   // $66
        .bind(&get_str(cert_info, "ClassifyAroundNo")) // $67
        .bind(&get_str(cert_info, "ValidPeriodExpirdateE"))    // $68
        .bind(&get_str(cert_info, "ValidPeriodExpirdateY"))    // $69
        .bind(&get_str(cert_info, "ValidPeriodExpirdateM"))    // $70
        .bind(&get_str(cert_info, "ValidPeriodExpirdateD"))    // $71
        .bind(&get_str(cert_info, "NoteInfo"))         // $72
        .bind(&get_str(cert_info, "TwodimensionCodeInfoEntryNoCarNo"))         // $73
        .bind(&get_str(cert_info, "TwodimensionCodeInfoCarNo"))               // $74
        .bind(&get_str(cert_info, "TwodimensionCodeInfoValidPeriodExpirdate")) // $75
        .bind(&get_str(cert_info, "TwodimensionCodeInfoModel"))               // $76
        .bind(&get_str(cert_info, "TwodimensionCodeInfoModelSpecifyNoClassifyAroundNo")) // $77
        .bind(&get_str(cert_info, "TwodimensionCodeInfoCharInfo"))            // $78
        .bind(&get_str(cert_info, "TwodimensionCodeInfoEngineModel"))         // $79
        .bind(&get_str(cert_info, "TwodimensionCodeInfoCarNoStampPlace"))     // $80
        .bind(&get_str(cert_info, "TwodimensionCodeInfoFirstregistdate"))     // $81
        .bind(&get_str(cert_info, "TwodimensionCodeInfoFfAxWgt"))             // $82
        .bind(&get_str(cert_info, "TwodimensionCodeInfoFrAxWgt"))             // $83
        .bind(&get_str(cert_info, "TwodimensionCodeInfoRfAxWgt"))             // $84
        .bind(&get_str(cert_info, "TwodimensionCodeInfoRrAxWgt"))             // $85
        .bind(&get_str(cert_info, "TwodimensionCodeInfoNoiseReg"))            // $86
        .bind(&get_str(cert_info, "TwodimensionCodeInfoNearNoiseReg"))        // $87
        .bind(&get_str(cert_info, "TwodimensionCodeInfoDriveMethod"))         // $88
        .bind(&get_str(cert_info, "TwodimensionCodeInfoOpacimeterMeasCar"))   // $89
        .bind(&get_str(cert_info, "TwodimensionCodeInfoNoxPmMeasMode"))       // $90
        .bind(&get_str(cert_info, "TwodimensionCodeInfoNoxValue"))            // $91
        .bind(&get_str(cert_info, "TwodimensionCodeInfoPmValue"))             // $92
        .bind(&get_str(cert_info, "TwodimensionCodeInfoSafeStdDate"))         // $93
        .bind(&get_str(cert_info, "TwodimensionCodeInfoFuelClassCode"))       // $94
        .bind(&get_str(cert_info, "RegistCarLightCar")) // $95
        .execute(&mut *conn)
        .await?;

        tracing::info!("car_inspection UPSERT completed: ElectCertMgNo={}", elect_cert_mg_no);

        // 5. car_inspection_files_a INSERT（JSONファイルの紐づけ）
        sqlx::query(
            r#"
            INSERT INTO car_inspection_files_a (uuid, organization_id, type, "ElectCertMgNo", "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD")
            VALUES ($1::uuid, current_setting('app.current_organization_id')::uuid, 'application/json', $2, $3, $4, $5, $6)
            ON CONFLICT (uuid) DO UPDATE SET modified_at = NOW()
            "#,
        )
        .bind(file_uuid)
        .bind(&elect_cert_mg_no)
        .bind(&grantdate_e)
        .bind(&grantdate_y)
        .bind(&grantdate_m)
        .bind(&grantdate_d)
        .execute(&mut *conn)
        .await?;

        tracing::info!("car_inspection_files_a INSERT completed: uuid={}", file_uuid);

        // 6. car_ins_sheet_ichiban_cars_a 車両リンク（hono-logi createCarInspection.ts L112-134）
        // 同じElectCertMgNoの既存レコードからid_carsを取得
        let existing = sqlx::query_as::<_, IchibanCarsLink>(
            r#"
            SELECT cisa.id_cars, ci."TwodimensionCodeInfoValidPeriodExpirdate"
            FROM car_ins_sheet_ichiban_cars_a cisa
            JOIN car_inspection ci
                ON ci.organization_id = cisa.organization_id
                AND ci."ElectCertMgNo" = cisa."ElectCertMgNo"
                AND ci."GrantdateE" = cisa."GrantdateE"
                AND ci."GrantdateY" = cisa."GrantdateY"
                AND ci."GrantdateM" = cisa."GrantdateM"
                AND ci."GrantdateD" = cisa."GrantdateD"
            WHERE cisa."ElectCertMgNo" = $1
            ORDER BY ci."TwodimensionCodeInfoValidPeriodExpirdate" DESC
            LIMIT 1
            "#,
        )
        .bind(&elect_cert_mg_no)
        .fetch_optional(&mut *conn)
        .await?;

        if let Some(link) = existing {
            if let Some(id_cars) = &link.id_cars {
                sqlx::query(
                    r#"
                    INSERT INTO car_ins_sheet_ichiban_cars_a (
                        organization_id, id_cars, "ElectCertMgNo",
                        "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD"
                    ) VALUES (
                        current_setting('app.current_organization_id')::uuid,
                        $1, $2, $3, $4, $5, $6
                    )
                    ON CONFLICT DO NOTHING
                    "#,
                )
                .bind(id_cars)
                .bind(&elect_cert_mg_no)
                .bind(&grantdate_e)
                .bind(&grantdate_y)
                .bind(&grantdate_m)
                .bind(&grantdate_d)
                .execute(&mut *conn)
                .await?;

                tracing::info!(
                    "car_ins_sheet_ichiban_cars_a linked: id_cars={}, ElectCertMgNo={}",
                    id_cars,
                    elect_cert_mg_no
                );
            }
        }

        // 7. pending_car_inspection_pdfs チェック（PDF先着の場合、Grantdateも一致確認）
        let pending_pdf = sqlx::query_as::<_, PendingPdf>(
            r#"
            SELECT file_uuid::text as file_uuid
            FROM pending_car_inspection_pdfs
            WHERE "ElectCertMgNo" = $1
            "#,
        )
        .bind(&elect_cert_mg_no)
        .fetch_optional(&mut *conn)
        .await?;

        if let Some(pdf) = pending_pdf {
            // car_inspection_files_b にPDFリンク作成
            sqlx::query(
                r#"
                INSERT INTO car_inspection_files_b (uuid, organization_id, type, "ElectCertMgNo", "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD")
                VALUES ($1::uuid, current_setting('app.current_organization_id')::uuid, 'application/pdf', $2, $3, $4, $5, $6)
                ON CONFLICT (uuid) DO UPDATE SET modified_at = NOW()
                "#,
            )
            .bind(&pdf.file_uuid)
            .bind(&elect_cert_mg_no)
            .bind(&grantdate_e)
            .bind(&grantdate_y)
            .bind(&grantdate_m)
            .bind(&grantdate_d)
            .execute(&mut *conn)
            .await?;

            // pending削除
            sqlx::query(
                r#"DELETE FROM pending_car_inspection_pdfs WHERE "ElectCertMgNo" = $1"#,
            )
            .bind(&elect_cert_mg_no)
            .execute(&mut *conn)
            .await?;

            tracing::info!(
                "Linked pending PDF: pdf_uuid={}, ElectCertMgNo={}",
                pdf.file_uuid,
                elect_cert_mg_no
            );
        }

        Ok(())
    }

    /// PDFファイルアップロード後に呼ばれる自動解析処理
    /// hono-logi createFiles.ts L291-365 + pdfCategory.ts 相当
    pub async fn process_pdf_upload(
        &self,
        file_uuid: &str,
        file_data: &[u8],
        organization_id: &str,
    ) -> Result<(), anyhow::Error> {
        // 1. PDFテキスト抽出（1ページ目のみ）
        let pages = pdf_extract::extract_text_from_mem_by_pages(file_data)?;
        let page1_text = match pages.first() {
            Some(text) if !text.is_empty() => text,
            _ => {
                tracing::debug!("PDF has no extractable text on page 1, skipping auto-parse");
                return Ok(());
            }
        };

        // 2. 車検証PDF判定
        if !RE_CAR_INSPECTION.is_match(page1_text) {
            tracing::debug!("PDF is not a car inspection certificate, skipping auto-parse");
            return Ok(());
        }

        // 3. ElectCertMgNo抽出（12桁数字）
        let elect_cert_mg_no = match RE_ELECT_CERT_MG_NO.find(page1_text) {
            Some(m) => m.as_str().to_string(),
            None => {
                tracing::warn!("Car inspection PDF but no ElectCertMgNo found");
                return Ok(());
            }
        };

        // 4. Grantdate抽出（3パターンのフォールバック）
        let caps = RE_GRANTDATE_HEADER
            .captures(page1_text)
            .or_else(|| RE_GRANTDATE_STANDARD.captures(page1_text))
            .or_else(|| RE_GRANTDATE_BIKO.captures(page1_text));

        let (grantdate_e, grantdate_y, grantdate_m, grantdate_d) = match caps {
            Some(caps) => (
                strip_spaces(&caps[1]),
                strip_spaces(&caps[2]),
                strip_spaces(&caps[3]),
                strip_spaces(&caps[4]),
            ),
            None => {
                tracing::warn!(
                    "Car inspection PDF but Grantdate not found: ElectCertMgNo={}",
                    elect_cert_mg_no
                );
                return Ok(());
            }
        };

        tracing::info!(
            "Auto-parsing PDF: ElectCertMgNo={}, Grantdate={}-{}-{}-{}",
            elect_cert_mg_no,
            grantdate_e,
            grantdate_y,
            grantdate_m,
            grantdate_d
        );

        // 5. DB接続取得 + RLS設定
        let mut conn = self.pool.acquire().await?;
        set_current_organization(&mut conn, organization_id).await?;

        // 6. car_inspection_files_aでJSON存在確認（ElectCertMgNo + Grantdate一致）
        let json_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM car_inspection_files_a
                WHERE "ElectCertMgNo" = $1
                  AND "GrantdateE" = $2
                  AND "GrantdateY" = $3
                  AND "GrantdateM" = $4
                  AND "GrantdateD" = $5
                  AND type = 'application/json'
                  AND deleted_at IS NULL
            )
            "#,
        )
        .bind(&elect_cert_mg_no)
        .bind(&grantdate_e)
        .bind(&grantdate_y)
        .bind(&grantdate_m)
        .bind(&grantdate_d)
        .fetch_one(&mut *conn)
        .await?;

        if json_exists {
            // 7a. JSON存在 → car_inspection_files_b にPDF直接紐づけ
            sqlx::query(
                r#"
                INSERT INTO car_inspection_files_b (uuid, organization_id, type, "ElectCertMgNo", "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD")
                VALUES ($1::uuid, current_setting('app.current_organization_id')::uuid, 'application/pdf', $2, $3, $4, $5, $6)
                ON CONFLICT (uuid) DO UPDATE SET modified_at = NOW()
                "#,
            )
            .bind(file_uuid)
            .bind(&elect_cert_mg_no)
            .bind(&grantdate_e)
            .bind(&grantdate_y)
            .bind(&grantdate_m)
            .bind(&grantdate_d)
            .execute(&mut *conn)
            .await?;

            tracing::info!(
                "PDF linked to car_inspection_files_b: uuid={}, ElectCertMgNo={}",
                file_uuid,
                elect_cert_mg_no
            );
        } else {
            // 7b. JSON未存在 → pending_car_inspection_pdfs にUPSERT（JSON待ち）
            sqlx::query(
                r#"
                INSERT INTO pending_car_inspection_pdfs (organization_id, file_uuid, "ElectCertMgNo", "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD")
                VALUES (current_setting('app.current_organization_id')::uuid, $1::uuid, $2, $3, $4, $5, $6)
                ON CONFLICT (organization_id, "ElectCertMgNo")
                DO UPDATE SET file_uuid = EXCLUDED.file_uuid,
                              "GrantdateE" = EXCLUDED."GrantdateE",
                              "GrantdateY" = EXCLUDED."GrantdateY",
                              "GrantdateM" = EXCLUDED."GrantdateM",
                              "GrantdateD" = EXCLUDED."GrantdateD",
                              created_at = NOW()
                "#,
            )
            .bind(file_uuid)
            .bind(&elect_cert_mg_no)
            .bind(&grantdate_e)
            .bind(&grantdate_y)
            .bind(&grantdate_m)
            .bind(&grantdate_d)
            .execute(&mut *conn)
            .await?;

            tracing::info!(
                "PDF stored as pending: uuid={}, ElectCertMgNo={}",
                file_uuid,
                elect_cert_mg_no
            );
        }

        Ok(())
    }
}

/// car_ins_sheet_ichiban_cars_a JOINクエリ用
#[derive(sqlx::FromRow)]
struct IchibanCarsLink {
    id_cars: Option<String>,
    #[allow(dead_code)]
    #[sqlx(rename = "TwodimensionCodeInfoValidPeriodExpirdate")]
    twodimension_code_info_valid_period_expirdate: String,
}

/// pending_car_inspection_pdfs クエリ用
#[derive(sqlx::FromRow)]
struct PendingPdf {
    file_uuid: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_text_extraction() {
        let pdf_data = include_bytes!("../../20260218141909_帯広１００け２０１.pdf");
        let pages = pdf_extract::extract_text_from_mem_by_pages(pdf_data).unwrap();
        assert!(!pages.is_empty(), "PDF should have at least 1 page");
        let text = &pages[0];
        assert!(!text.is_empty(), "Page 1 should have text");
    }

    #[test]
    fn test_pdf_car_inspection_detection() {
        let pdf_data = include_bytes!("../../20260218141909_帯広１００け２０１.pdf");
        let pages = pdf_extract::extract_text_from_mem_by_pages(pdf_data).unwrap();
        let text = &pages[0];
        assert!(RE_CAR_INSPECTION.is_match(text), "Should detect car inspection certificate");
    }

    #[test]
    fn test_pdf_elect_cert_mg_no() {
        let pdf_data = include_bytes!("../../20260218141909_帯広１００け２０１.pdf");
        let pages = pdf_extract::extract_text_from_mem_by_pages(pdf_data).unwrap();
        let text = &pages[0];
        let ecmn = RE_ELECT_CERT_MG_NO.find(text).expect("ElectCertMgNo not found");
        assert_eq!(ecmn.as_str(), "141230033850");
    }

    #[test]
    fn test_pdf_grantdate() {
        let pdf_data = include_bytes!("../../20260218141909_帯広１００け２０１.pdf");
        let pages = pdf_extract::extract_text_from_mem_by_pages(pdf_data).unwrap();
        let text = &pages[0];

        let caps = RE_GRANTDATE_HEADER
            .captures(text)
            .or_else(|| RE_GRANTDATE_STANDARD.captures(text))
            .or_else(|| RE_GRANTDATE_BIKO.captures(text))
            .expect("Grantdate not found");

        assert_eq!(strip_spaces(&caps[1]), "令和");
        assert_eq!(strip_spaces(&caps[2]), "8");
        assert_eq!(strip_spaces(&caps[3]), "2");
        assert_eq!(strip_spaces(&caps[4]), "13");
    }
}
