# @rust-logi/proto

rust-logi gRPC-WebサービスのProtocol Buffersと生成済みTypeScript。

## インストール

```bash
npm install ../rust-logi/packages/logi-proto
```

または `package.json` に追加:

```json
{
  "dependencies": {
    "@rust-logi/proto": "file:../rust-logi/packages/logi-proto"
  }
}
```

## 使い方

```typescript
import { File, FilesService, CreateFileRequest } from "@rust-logi/proto";
import { createClient } from "@connectrpc/connect";
import { createGrpcWebTransport } from "@connectrpc/connect-web";

// gRPC-Webトランスポートを作成
const transport = createGrpcWebTransport({
  baseUrl: "http://localhost:50051",
});

// クライアントを作成
const client = createClient(FilesService, transport);

// ファイル一覧を取得
const response = await client.listFiles({});
console.log(response.files);

// ファイルを作成
const file = await client.createFile({
  filename: "test.txt",
  type: "text/plain",
  blobBase64: btoa("Hello, World!"),
});
```

## 含まれるサービス

| サービス | 説明 |
|----------|------|
| `FilesService` | ファイル管理 (CRUD, ダウンロード) |
| `CamFilesService` | CAMファイル管理 |
| `CamFileExeStageService` | CAMファイル実行ステージ |
| `CarInspectionService` | 車検データ管理 |
| `CarInspectionFilesService` | 車検ファイル管理 |
| `Health` | ヘルスチェック |

## 開発

### ビルド

```bash
npm install
npm run build
```

### proto生成のみ

```bash
npm run generate
```

### クリーン

```bash
npm run clean
```

## ファイル構成

```
packages/logi-proto/
├── proto/           # Protoファイル
├── src/
│   ├── gen/         # 生成済みTypeScript (gitignore)
│   └── index.ts     # エクスポート
├── dist/            # コンパイル済みJS (gitignore)
├── buf.yaml         # Buf設定
├── buf.gen.yaml     # コード生成設定
└── tsconfig.json
```
