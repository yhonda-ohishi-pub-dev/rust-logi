-- Migration: Create car_inspection related tables with organization support

-- Main car inspection table
CREATE TABLE car_inspection (
    id SERIAL,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    "CertInfoImportFileVersion" TEXT NOT NULL,
    "Acceptoutputno" TEXT NOT NULL,
    "FormType" TEXT NOT NULL,
    "ElectCertMgNo" TEXT NOT NULL,
    "CarId" TEXT NOT NULL,
    "ElectCertPublishdateE" TEXT NOT NULL,
    "ElectCertPublishdateY" TEXT NOT NULL,
    "ElectCertPublishdateM" TEXT NOT NULL,
    "ElectCertPublishdateD" TEXT NOT NULL,
    "GrantdateE" TEXT NOT NULL,
    "GrantdateY" TEXT NOT NULL,
    "GrantdateM" TEXT NOT NULL,
    "GrantdateD" TEXT NOT NULL,
    "TranspotationBureauchiefName" TEXT NOT NULL,
    "EntryNoCarNo" TEXT NOT NULL,
    "ReggrantdateE" TEXT NOT NULL,
    "ReggrantdateY" TEXT NOT NULL,
    "ReggrantdateM" TEXT NOT NULL,
    "ReggrantdateD" TEXT NOT NULL,
    "FirstregistdateE" TEXT NOT NULL,
    "FirstregistdateY" TEXT NOT NULL,
    "FirstregistdateM" TEXT NOT NULL,
    "CarName" TEXT NOT NULL,
    "CarNameCode" TEXT NOT NULL,
    "CarNo" TEXT NOT NULL,
    "Model" TEXT NOT NULL,
    "EngineModel" TEXT NOT NULL,
    "OwnernameLowLevelChar" TEXT NOT NULL,
    "OwnernameHighLevelChar" TEXT NOT NULL,
    "OwnerAddressChar" TEXT NOT NULL,
    "OwnerAddressNumValue" TEXT NOT NULL,
    "OwnerAddressCode" TEXT NOT NULL,
    "UsernameLowLevelChar" TEXT NOT NULL,
    "UsernameHighLevelChar" TEXT NOT NULL,
    "UserAddressChar" TEXT NOT NULL,
    "UserAddressNumValue" TEXT NOT NULL,
    "UserAddressCode" TEXT NOT NULL,
    "UseheadqrterChar" TEXT NOT NULL,
    "UseheadqrterNumValue" TEXT NOT NULL,
    "UseheadqrterCode" TEXT NOT NULL,
    "CarKind" TEXT NOT NULL,
    "Use" TEXT NOT NULL,
    "PrivateBusiness" TEXT NOT NULL,
    "CarShape" TEXT NOT NULL,
    "CarShapeCode" TEXT NOT NULL,
    "NoteCap" TEXT NOT NULL,
    "Cap" TEXT NOT NULL,
    "NoteMaxloadage" TEXT NOT NULL,
    "Maxloadage" TEXT NOT NULL,
    "NoteCarWgt" TEXT NOT NULL,
    "CarWgt" TEXT NOT NULL,
    "NoteCarTotalWgt" TEXT NOT NULL,
    "CarTotalWgt" TEXT NOT NULL,
    "NoteLength" TEXT NOT NULL,
    "Length" TEXT NOT NULL,
    "NoteWidth" TEXT NOT NULL,
    "Width" TEXT NOT NULL,
    "NoteHeight" TEXT NOT NULL,
    "Height" TEXT NOT NULL,
    "FfAxWgt" TEXT NOT NULL,
    "FrAxWgt" TEXT NOT NULL,
    "RfAxWgt" TEXT NOT NULL,
    "RrAxWgt" TEXT NOT NULL,
    "Displacement" TEXT NOT NULL,
    "FuelClass" TEXT NOT NULL,
    "ModelSpecifyNo" TEXT NOT NULL,
    "ClassifyAroundNo" TEXT NOT NULL,
    "ValidPeriodExpirdateE" TEXT NOT NULL,
    "ValidPeriodExpirdateY" TEXT NOT NULL,
    "ValidPeriodExpirdateM" TEXT NOT NULL,
    "ValidPeriodExpirdateD" TEXT NOT NULL,
    "NoteInfo" TEXT NOT NULL,
    "TwodimensionCodeInfoEntryNoCarNo" TEXT NOT NULL,
    "TwodimensionCodeInfoCarNo" TEXT NOT NULL,
    "TwodimensionCodeInfoValidPeriodExpirdate" TEXT NOT NULL,
    "TwodimensionCodeInfoModel" TEXT NOT NULL,
    "TwodimensionCodeInfoModelSpecifyNoClassifyAroundNo" TEXT NOT NULL,
    "TwodimensionCodeInfoCharInfo" TEXT NOT NULL,
    "TwodimensionCodeInfoEngineModel" TEXT NOT NULL,
    "TwodimensionCodeInfoCarNoStampPlace" TEXT NOT NULL,
    "TwodimensionCodeInfoFirstregistdate" TEXT NOT NULL,
    "TwodimensionCodeInfoFfAxWgt" TEXT NOT NULL,
    "TwodimensionCodeInfoFrAxWgt" TEXT NOT NULL,
    "TwodimensionCodeInfoRfAxWgt" TEXT NOT NULL,
    "TwodimensionCodeInfoRrAxWgt" TEXT NOT NULL,
    "TwodimensionCodeInfoNoiseReg" TEXT NOT NULL,
    "TwodimensionCodeInfoNearNoiseReg" TEXT NOT NULL,
    "TwodimensionCodeInfoDriveMethod" TEXT NOT NULL,
    "TwodimensionCodeInfoOpacimeterMeasCar" TEXT NOT NULL,
    "TwodimensionCodeInfoNoxPmMeasMode" TEXT NOT NULL,
    "TwodimensionCodeInfoNoxValue" TEXT NOT NULL,
    "TwodimensionCodeInfoPmValue" TEXT NOT NULL,
    "TwodimensionCodeInfoSafeStdDate" TEXT NOT NULL,
    "TwodimensionCodeInfoFuelClassCode" TEXT NOT NULL,
    "RegistCarLightCar" TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id),
    CONSTRAINT car_inspection_org_unique UNIQUE (organization_id, "ElectCertMgNo", "GrantdateE", "GrantdateY", "GrantdateM", "GrantdateD")
);

CREATE INDEX idx_car_inspection_organization_id ON car_inspection(organization_id);
CREATE INDEX idx_car_inspection_car_id ON car_inspection(organization_id, "CarId");

ALTER TABLE car_inspection ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON car_inspection
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- Car inspection files table (uses ElectCertPublishdate, not Grantdate)
CREATE TABLE car_inspection_files (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    type TEXT NOT NULL,
    "ElectCertMgNo" TEXT NOT NULL,
    "ElectCertPublishdateE" TEXT NOT NULL,
    "ElectCertPublishdateY" TEXT NOT NULL,
    "ElectCertPublishdateM" TEXT NOT NULL,
    "ElectCertPublishdateD" TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    modified_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_car_inspection_files_organization_id ON car_inspection_files(organization_id);

ALTER TABLE car_inspection_files ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON car_inspection_files
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- Car inspection deregistration table
CREATE TABLE car_inspection_deregistration (
    id SERIAL,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    "CarId" TEXT NOT NULL,
    "TwodimensionCodeInfoCarNo" TEXT NOT NULL,
    "CarNo" TEXT NOT NULL,
    "ValidPeriodExpirdateE" TEXT NOT NULL,
    "ValidPeriodExpirdateY" TEXT NOT NULL,
    "ValidPeriodExpirdateM" TEXT NOT NULL,
    "ValidPeriodExpirdateD" TEXT NOT NULL,
    "TwodimensionCodeInfoValidPeriodExpirdate" TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id),
    CONSTRAINT car_inspection_dereg_org_unique UNIQUE (
        organization_id, "CarId", "TwodimensionCodeInfoCarNo",
        "ValidPeriodExpirdateE", "ValidPeriodExpirdateY",
        "ValidPeriodExpirdateM", "ValidPeriodExpirdateD"
    )
);

CREATE INDEX idx_car_inspection_dereg_organization_id ON car_inspection_deregistration(organization_id);

ALTER TABLE car_inspection_deregistration ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON car_inspection_deregistration
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- Car inspection deregistration files table
CREATE TABLE car_inspection_deregistration_files (
    id SERIAL PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    "CarId" TEXT NOT NULL,
    "TwodimensionCodeInfoValidPeriodExpirdate" TEXT NOT NULL,
    file_uuid UUID NOT NULL REFERENCES files(uuid),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT car_inspection_dereg_files_unique UNIQUE (
        organization_id, "CarId", "TwodimensionCodeInfoValidPeriodExpirdate", file_uuid
    )
);

CREATE INDEX idx_car_inspection_dereg_files_organization_id ON car_inspection_deregistration_files(organization_id);

ALTER TABLE car_inspection_deregistration_files ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON car_inspection_deregistration_files
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));
