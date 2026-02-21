# LINE WORKS Rich Menu API v2 仕様

## 概要

LINE WORKS Bot のトーク画面下部に表示されるリッチメニューを管理するAPI。
リッチメニューはモバイルアプリでのみ画像が表示され、PCアプリでは label テキストが表示される。

## Base URL

```
https://www.worksapis.com/v1.0/bots/{botId}
```

## 認証

- Header: `Authorization: Bearer {access_token}`
- Scope: `bot.message`, `bot`（リスト取得のみ `bot.read` も可）

### Access Token 取得フロー

1. JWT 生成（RS256署名）
   - Claims: `{ iss: clientId, sub: serviceAccount, iat, exp: iat+60 }`
2. トークンエンドポイントに POST
   ```
   POST https://auth.worksmobile.com/oauth2/v2.0/token
   Content-Type: application/x-www-form-urlencoded

   assertion={jwt}
   &grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer
   &client_id={clientId}
   &client_secret={clientSecret}
   &scope=bot
   ```
3. レスポンス: `{ access_token, refresh_token, scope, token_type, expires_in }`

## API エンドポイント

### 1. リッチメニュー登録

```
POST /richmenus
Content-Type: application/json
```

**Request Body:**

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| richmenuName | string | Yes | メニュー名（maxLength: 300） |
| areas | array(RichmenuArea) | Yes | タップ領域（1〜20個） |
| size | object(Size) | Yes | メニューサイズ |

**Response:** HTTP 201

```json
{
  "richmenuId": "40001",
  "richmenuName": "メインメニュー",
  "areas": [...],
  "size": { "width": 2500, "height": 843 }
}
```

### 2. リッチメニューリスト取得

```
GET /richmenus?count={count}&cursor={cursor}
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| count | integer | Yes | 取得数（default: 50, min: 1, max: 100） |
| cursor | string | No | ページネーション用カーソル（URL エンコード） |

**Response:** HTTP 200

```json
{
  "richmenus": [
    {
      "richmenuId": "40001",
      "richmenuName": "メインメニュー",
      "areas": [...],
      "size": { "width": 2500, "height": 843 }
    }
  ],
  "responseMetaData": { ... }
}
```

### 3. リッチメニュー取得

```
GET /richmenus/{richmenuId}
```

**Response:** HTTP 200 — リッチメニューオブジェクト

### 4. リッチメニュー削除

```
DELETE /richmenus/{richmenuId}
```

**Response:** HTTP 204 No Content

### 5. コンテンツアップロード（画像アップロード準備）

```
POST /attachments
Content-Type: application/json
```

**Request Body:**

```json
{
  "fileName": "richmenu.png"
}
```

**Response:** HTTP 200

```json
{
  "fileId": "jp1.1628695315008671000.1628781715.0.1000001.0.0.0",
  "uploadUrl": "https://storage.worksmobile.com/k/emsg/r/jp1/..."
}
```

**注意:**
- uploadUrl / fileId の有効期限は **24時間**
- 一度アップロードした URL は再利用不可

### 6. 画像バイナリアップロード

```
PUT {uploadUrl}
Content-Type: image/png (or image/jpeg)
Body: <バイナリデータ>
```

### 7. リッチメニュー画像登録

```
POST /richmenus/{richmenuId}/image
Content-Type: application/json
```

**Request Body:**

```json
{
  "fileId": "jp1.1628695315008671000.1628781715.0.1000001.0.0.0"
}
```

**Response:** HTTP 204 No Content

**画像要件:**
- 形式: JPEG / PNG
- サイズ: **2500x1686** または **2500x843** ピクセル
- 最大ファイルサイズ: **1MB**
- モバイルアプリでのみ表示（PC版では label テキストが表示）

### 8. デフォルトリッチメニュー適用

```
POST /richmenus/{richmenuId}/set-default
```

**Response:** HTTP 201

```json
{
  "botId": 2000001,
  "defaultRichmenuId": "40001"
}
```

**表示優先順位:**
1. ユーザー別リッチメニュー
2. デフォルトリッチメニュー

### 9. デフォルトリッチメニュー取得

```
GET /richmenus/default
```

**Response:** HTTP 200

```json
{
  "botId": 2000001,
  "defaultRichmenuId": "40001"
}
```

### 10. デフォルトリッチメニュー削除

```
DELETE /richmenus/default
```

**Response:** HTTP 204 No Content

## データモデル

### RichmenuArea

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| action | object(Action) | Yes | エリアを押した時の動作 |
| bounds | object(Bounds) | Yes | タップ領域の座標・サイズ |

### Action

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| type | string | Yes | `postback`, `message`, `uri`, `copy` |
| label | string | No | PC版で表示されるラベル（max: 20字） |
| data | string | No | postback.data として返す（maxLength: 300） |
| displayText | string | No | トーク画面に表示（maxLength: 300） |
| postback | string | No | message.postback として返す（maxLength: 1000） |
| text | string | No | タップ時に送信されるテキスト（maxLength: 300） |
| uri | string | No | タップ時に開くURL（http/https、maxLength: 1000） |
| copyText | string | No | クリップボードにコピー（maxLength: 1000） |

### Bounds

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| x | integer | Yes | 左端からの水平位置（0〜2500） |
| y | integer | Yes | 上端からの垂直位置（0〜1686） |
| width | integer | Yes | 幅（0〜2500） |
| height | integer | Yes | 高さ（0〜1686） |

### Size

| Property | Type | Required | Allowed Values |
|----------|------|----------|----------------|
| width | integer | Yes | **2500** のみ |
| height | integer | Yes | **843** or **1686** |

## リッチメニュー作成手順

1. `POST /richmenus` — メニュー定義を登録 → `richmenuId` 取得
2. `POST /attachments` — アップロードURL取得 → `{ fileId, uploadUrl }`
3. `PUT {uploadUrl}` — 画像バイナリをアップロード
4. `POST /richmenus/{richmenuId}/image` — fileId で画像を紐付け
5. `POST /richmenus/{richmenuId}/set-default` — デフォルトに適用

## サンプル: 2分割メニュー（アプリリンク）

```json
{
  "richmenuName": "アプリメニュー",
  "size": { "width": 2500, "height": 843 },
  "areas": [
    {
      "bounds": { "x": 0, "y": 0, "width": 1250, "height": 843 },
      "action": {
        "type": "uri",
        "label": "車検証管理",
        "uri": "https://carins.mtamaramu.com/?lw=ohishi"
      }
    },
    {
      "bounds": { "x": 1250, "y": 0, "width": 1250, "height": 843 },
      "action": {
        "type": "uri",
        "label": "DTako ログ",
        "uri": "https://ohishi2.mtamaramu.com/?lw=ohishi"
      }
    }
  ]
}
```

## 管理画面

`https://auth.mtamaramu.com/admin/rich-menu` — auth-worker で実装。
LINE WORKS API を直接呼び出してリッチメニューの CRUD + 画像管理 + デフォルト設定を行う。
