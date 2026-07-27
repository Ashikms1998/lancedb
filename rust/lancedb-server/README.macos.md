# LanceDB server for Apple Silicon macOS

This package contains an unsigned, native Apple Silicon (`arm64`) executable with
LanceDB embedded directly inside it. It exposes the same HTTP API as the Windows build,
including ping, database health, table management, row CRUD, and vector search.

The TypeScript server communicates with it over HTTP. No Python, Node.js, separate
LanceDB service, or embedding model is included in the binary.

## Supported system

- Apple Silicon Mac: M1, M2, M3, M4, or newer
- macOS 11 or newer
- A writable directory for LanceDB data
- Port `8080` available on localhost

This is not an Intel or universal binary.

## Package contents

The CI or build script produces:

```text
lancedb-server-macos-arm64/
├── lancedb-server-macos-arm64
├── README.md
└── SHA256SUMS
```

## Step 1: Extract and verify the package

```bash
tar -xzf lancedb-server-macos-arm64.tar.gz
cd lancedb-server-macos-arm64
shasum -a 256 -c SHA256SUMS
```

Expected checksum result:

```text
lancedb-server-macos-arm64: OK
```

## Step 2: Allow the executable to run

The packaged file already has executable permission, but it is safe to apply it again:

```bash
chmod +x ./lancedb-server-macos-arm64
```

The test build is unsigned. If macOS reports that Apple cannot check it for malicious
software, remove quarantine only after verifying the checksum and trusting the source:

```bash
xattr -d com.apple.quarantine ./lancedb-server-macos-arm64
```

Signing and notarization should be added before public distribution.

## Step 3: Start the server

Open terminal 1:

```bash
mkdir -p "$HOME/lancedb-test-data"

export LANCEDB_DATA_DIR="$HOME/lancedb-test-data"
export LANCEDB_BIND="127.0.0.1:8080"

./lancedb-server-macos-arm64
```

Expected output:

```text
LanceDB server listening on http://127.0.0.1:8080
```

Keep terminal 1 open while testing. Press `Control+C` to stop the server.

## Step 4: Configure the test terminal

Open terminal 2:

```bash
BASE_URL="http://127.0.0.1:8080/v1"
```

Run the remaining commands in terminal 2, one at a time and in order.

## Step 5: Test ping

This verifies that the TypeScript server can reach the process using the same endpoint.

```bash
curl --fail-with-body --silent --show-error "$BASE_URL/ping"
```

Expected response:

```json
{"ok":true,"data":{"message":"pong"}}
```

## Step 6: Test database health

```bash
curl --fail-with-body --silent --show-error "$BASE_URL/db/health"
```

Expected response:

```json
{"ok":true,"data":{"status":"ok","database":"connected"}}
```

## Step 7: List tables

```bash
curl --fail-with-body --silent --show-error "$BASE_URL/tables"
```

A new data directory should return an empty `tables` array.

## Step 8: Create a table

Create `documents` with four-dimensional vectors:

```bash
curl --fail-with-body --silent --show-error \
  --request POST \
  --header "content-type: application/json" \
  --data '{"name":"documents","vector_dimension":4}' \
  "$BASE_URL/tables"
```

Expected result: HTTP `201 Created` and table name `documents`.

Confirm the table exists:

```bash
curl --fail-with-body --silent --show-error "$BASE_URL/tables"
```

## Step 9: Insert rows

Every vector must contain exactly four numbers:

```bash
curl --fail-with-body --silent --show-error \
  --request POST \
  --header "content-type: application/json" \
  --data '{
    "rows": [
      {"id":"doc-1","text":"first document","vector":[1.0,0.0,0.0,0.0]},
      {"id":"doc-2","text":"second document","vector":[0.0,1.0,0.0,0.0]},
      {"id":"doc-3","text":"third document","vector":[0.0,0.0,1.0,0.0]}
    ]
  }' \
  "$BASE_URL/tables/documents/rows"
```

Expected result: `count` is `3`.

## Step 10: Read rows

```bash
curl --fail-with-body --silent --show-error \
  "$BASE_URL/tables/documents/rows?limit=10"
```

The maximum read limit is `1000`; the default is `100`.

## Step 11: Count rows

```bash
curl --fail-with-body --silent --show-error \
  "$BASE_URL/tables/documents/count"
```

Expected result: `count` is `3`.

## Step 12: Run vector search

The query below should return `doc-1` first:

```bash
curl --fail-with-body --silent --show-error \
  --request POST \
  --header "content-type: application/json" \
  --data '{"vector":[1.0,0.0,0.0,0.0],"limit":2}' \
  "$BASE_URL/tables/documents/search"
```

Each result includes `id`, `text`, `vector`, and `distance`.

## Step 13: Update a row

The minimal API updates the text field using the document ID:

```bash
curl --fail-with-body --silent --show-error \
  --request PATCH \
  --header "content-type: application/json" \
  --data '{"text":"first document updated"}' \
  "$BASE_URL/tables/documents/rows/doc-1"
```

Expected result: `count` is `1`.

Read the rows again to verify the updated text:

```bash
curl --fail-with-body --silent --show-error \
  "$BASE_URL/tables/documents/rows?limit=10"
```

## Step 14: Delete a row

```bash
curl --fail-with-body --silent --show-error \
  --request DELETE \
  "$BASE_URL/tables/documents/rows/doc-2"
```

Expected result: `count` is `1`.

Confirm that two rows remain:

```bash
curl --fail-with-body --silent --show-error \
  "$BASE_URL/tables/documents/count"
```

## Step 15: Drop the test table

This permanently removes the test table:

```bash
curl --fail-with-body --silent --show-error \
  --request DELETE \
  --write-out "HTTP %{http_code}\n" \
  "$BASE_URL/tables/documents"
```

Expected status: `HTTP 204`.

Confirm that the table is gone:

```bash
curl --fail-with-body --silent --show-error "$BASE_URL/tables"
```

## Step 16: Stop the server

Return to terminal 1 and press `Control+C`. Database files remain in:

```text
~/lancedb-test-data
```

## Test from a TypeScript server

Node.js 18 or newer provides the global `fetch` used below:

```typescript
const response = await fetch("http://127.0.0.1:8080/v1/ping");
if (!response.ok) {
  throw new Error(`LanceDB server returned HTTP ${response.status}`);
}

const result = await response.json();
console.log(result.data.message); // pong
```

The complete typed client example is in `rust/lancedb-server/examples/client.ts` in
the source repository. The API paths and JSON payloads are identical on Windows and
macOS.

## Build on an Apple Silicon Mac

Install the prerequisites:

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
brew install protobuf
```

From the source repository root:

```bash
bash rust/lancedb-server/scripts/build-macos-arm64.sh
```

The script performs all of the following:

1. Verifies that the host is Apple Silicon macOS.
2. Verifies that Cargo, Rustup, and `protoc` are installed.
3. Runs the complete HTTP-to-LanceDB integration test.
4. Creates the optimized `aarch64-apple-darwin` binary.
5. Confirms that the output is an ARM64 Mach-O executable.
6. Creates a checksum and distributable archive.

Output:

```text
dist/lancedb-server-macos-arm64.tar.gz
dist/lancedb-server-macos-arm64.tar.gz.sha256
```

## GitHub Actions build

The workflow `.github/workflows/lancedb-server-macos.yml` runs on a native Apple
Silicon GitHub runner. It runs the integration test, builds the release binary, and
uploads `lancedb-server-macos-arm64` as an unsigned test artifact.

You can run it manually from GitHub:

1. Open the repository's **Actions** tab.
2. Select **LanceDB server macOS ARM64**.
3. Select **Run workflow**.
4. Download the `lancedb-server-macos-arm64` artifact when the job completes.

## Current security boundary

The server binds to `127.0.0.1` by default and is intended for local or trusted test
environments. It has no authentication or TLS. Do not expose it to a public network.
