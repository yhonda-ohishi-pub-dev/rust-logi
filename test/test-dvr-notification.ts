import { createClient } from "@connectrpc/connect";
import { createGrpcWebTransport } from "@connectrpc/connect-web";
import {
  DvrNotificationsService,
  BulkCreateDvrNotificationsRequestSchema,
  DvrNotificationSchema,
} from "@yhonda-ohishi-pub-dev/logi-proto";
import { create } from "@bufbuild/protobuf";
import { execSync } from "child_process";

const RUST_LOGI_URL = "https://rust-logi-747065218280.asia-northeast1.run.app";
const ORGANIZATION_ID = "00000000-0000-0000-0000-000000000001"; // Default Organization

async function getIdToken(): Promise<string> {
  const token = execSync("gcloud auth print-identity-token", { encoding: "utf-8" }).trim();
  return token;
}

async function main() {
  const idToken = await getIdToken();
  console.log("Got ID token (first 50 chars):", idToken.substring(0, 50) + "...");

  const transport = createGrpcWebTransport({
    baseUrl: RUST_LOGI_URL,
  });

  const client = createClient(DvrNotificationsService, transport);

  // テスト用のDVR通知データ
  const testNotification = create(DvrNotificationSchema, {
    vehicleCd: BigInt(12345),
    vehicleName: "テスト車両001",
    serialNo: "SN-TEST-001",
    fileName: "test_video.mp4",
    eventType: "急ブレーキ",
    dvrDatetime: new Date().toISOString(),
    driverName: "テストドライバー",
    mp4Url: `https://example.com/test-${Date.now()}.mp4`, // ユニークなURLにする
  });

  const request = create(BulkCreateDvrNotificationsRequestSchema, {
    notifications: [testNotification],
  });

  console.log("\nSending DVR notification to:", RUST_LOGI_URL);
  console.log("Request:", JSON.stringify(request, (_, v) => typeof v === 'bigint' ? v.toString() : v, 2));

  try {
    const response = await client.bulkCreate(request, {
      headers: {
        "x-organization-id": ORGANIZATION_ID,
        "Authorization": `Bearer ${idToken}`,
      },
    });

    console.log("\nResponse:");
    console.log("  success:", response.success);
    console.log("  recordsAdded:", response.recordsAdded);
    console.log("  totalRecords:", response.totalRecords);
    console.log("  message:", response.message);

    if (response.success && response.recordsAdded > 0) {
      console.log("\n✅ DVR notification created and LINE notification should be sent!");
    } else if (response.recordsAdded === 0) {
      console.log("\n⚠️ No records added (possibly duplicate mp4_url)");
    }
  } catch (error) {
    console.error("\n❌ Error:", error);
    process.exit(1);
  }
}

main();
