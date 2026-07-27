# Minimal LanceDB server

A small Windows HTTP server with LanceDB embedded directly in the executable. The
server uses a fixed document schema containing `id`, `text`, and a client-generated
`vector`. It does not include embedding models, TLS, authentication, or LanceDB cloud
support.

For Apple Silicon setup, packaging, and complete API testing, see
[`README.macos.md`](README.macos.md).


```

You can run it from that location or copy it to another directory. The database data
is stored separately in the directory configured through `LANCEDB_DATA_DIR`.

## Prerequisites

- Windows PowerShell 5.1 or PowerShell 7
- The `lancedb-server.exe` file
- A writable directory for LanceDB data
- Port `8080` available on localhost

The API examples below use PowerShell's built-in `Invoke-RestMethod`, so `curl` is not
required.

## Step 1: Start the server

Open PowerShell terminal 1 and run:

```powershell
New-Item -ItemType Directory -Force -Path "E:\lance-db-data" | Out-Null

$env:LANCEDB_DATA_DIR = "E:\lance-db-data"
$env:LANCEDB_BIND = "127.0.0.1:8080"

& "E:\lance-db\lancedb\target\release\lancedb-server.exe"
```

Expected output:

```text
LanceDB server listening on http://127.0.0.1:8080
```

Keep terminal 1 open while testing. Closing it or pressing `Ctrl+C` stops the server.

The server binds to `127.0.0.1`, so only programs on the same computer can reach it.

## Step 2: Open a test terminal

Open PowerShell terminal 2. Define the API address once:

```powershell
$baseUrl = "http://127.0.0.1:8080/v1"
```

Run all remaining commands in terminal 2, one at a time and in the order shown.

## Step 3: Test ping

This confirms that the executable is running and HTTP communication works.

```powershell
Invoke-RestMethod -Method Get -Uri "$baseUrl/ping" |
    ConvertTo-Json -Depth 10
```

Expected response:

```json
{
  "ok": true,
  "data": {
    "message": "pong"
  }
}
```

## Step 4: Test database health

This confirms that the server can access the configured LanceDB data directory.

```powershell
Invoke-RestMethod -Method Get -Uri "$baseUrl/db/health" |
    ConvertTo-Json -Depth 10
```

Expected response:

```json
{
  "ok": true,
  "data": {
    "status": "ok",
    "database": "connected"
  }
}
```

## Step 5: List tables before creating one

```powershell
Invoke-RestMethod -Method Get -Uri "$baseUrl/tables" |
    ConvertTo-Json -Depth 10
```

For a new data directory, `tables` should be empty.

## Step 6: Create a table

This test creates a table named `documents` with four-dimensional vectors.

```powershell
$createTableBody = @{
    name = "documents"
    vector_dimension = 4
} | ConvertTo-Json

Invoke-RestMethod `
    -Method Post `
    -Uri "$baseUrl/tables" `
    -ContentType "application/json" `
    -Body $createTableBody |
    ConvertTo-Json -Depth 10
```

Expected result: HTTP `201 Created` and table name `documents`.

The table name may contain letters, numbers, `_`, and `-`. The vector dimension must
be greater than zero.

## Step 7: List tables again

```powershell
Invoke-RestMethod -Method Get -Uri "$baseUrl/tables" |
    ConvertTo-Json -Depth 10
```

The response should now contain `documents`.

## Step 8: Insert rows

Every vector must contain exactly four numbers because the table was created with
`vector_dimension = 4`.

```powershell
$insertBody = @{
    rows = @(
        @{
            id = "doc-1"
            text = "first document"
            vector = @(1.0, 0.0, 0.0, 0.0)
        },
        @{
            id = "doc-2"
            text = "second document"
            vector = @(0.0, 1.0, 0.0, 0.0)
        },
        @{
            id = "doc-3"
            text = "third document"
            vector = @(0.0, 0.0, 1.0, 0.0)
        }
    )
} | ConvertTo-Json -Depth 10

Invoke-RestMethod `
    -Method Post `
    -Uri "$baseUrl/tables/documents/rows" `
    -ContentType "application/json" `
    -Body $insertBody |
    ConvertTo-Json -Depth 10
```

Expected result: `count` is `3`.

## Step 9: Read rows

```powershell
Invoke-RestMethod -Method Get -Uri "$baseUrl/tables/documents/rows?limit=10" |
    ConvertTo-Json -Depth 10
```

The maximum accepted read limit is `1000`; the default is `100`.

## Step 10: Count rows

```powershell
Invoke-RestMethod -Method Get -Uri "$baseUrl/tables/documents/count" |
    ConvertTo-Json -Depth 10
```

Expected result: `count` is `3`.

## Step 11: Run vector search

The following query is closest to `doc-1`.

```powershell
$searchBody = @{
    vector = @(1.0, 0.0, 0.0, 0.0)
    limit = 2
} | ConvertTo-Json

Invoke-RestMethod `
    -Method Post `
    -Uri "$baseUrl/tables/documents/search" `
    -ContentType "application/json" `
    -Body $searchBody |
    ConvertTo-Json -Depth 10
```

Expected result: the first row is `doc-1` and includes a `_distance` value returned as
`distance`. Search limits are restricted to the range `1` through `100`.

## Step 12: Update a row

The current minimal API updates the `text` field using the document ID.

```powershell
$updateBody = @{
    text = "first document updated"
} | ConvertTo-Json

Invoke-RestMethod `
    -Method Patch `
    -Uri "$baseUrl/tables/documents/rows/doc-1" `
    -ContentType "application/json" `
    -Body $updateBody |
    ConvertTo-Json -Depth 10
```

Expected result: `count` is `1`.

Read the rows again to confirm the change:

```powershell
Invoke-RestMethod -Method Get -Uri "$baseUrl/tables/documents/rows?limit=10" |
    ConvertTo-Json -Depth 10
```

## Step 13: Delete a row

```powershell
Invoke-RestMethod `
    -Method Delete `
    -Uri "$baseUrl/tables/documents/rows/doc-2" |
    ConvertTo-Json -Depth 10
```

Expected result: `count` is `1`.

Confirm that two rows remain:

```powershell
Invoke-RestMethod -Method Get -Uri "$baseUrl/tables/documents/count" |
    ConvertTo-Json -Depth 10
```

## Step 14: Drop the test table

This permanently deletes the `documents` test table from the configured data
directory.

```powershell
Invoke-RestMethod -Method Delete -Uri "$baseUrl/tables/documents"
```

A successful request returns HTTP `204 No Content`, so PowerShell prints no JSON.

Confirm that the table is gone:

```powershell
Invoke-RestMethod -Method Get -Uri "$baseUrl/tables" |
    ConvertTo-Json -Depth 10
```

## Step 15: Stop the server

Return to terminal 1 and press:

```text
Ctrl+C
```

The LanceDB data remains in `E:\lance-db-data` for the next server run unless you
explicitly delete that directory.

## TypeScript usage

A typed example client is available at
[`examples/client.ts`](examples/client.ts). Copy it into the TypeScript server or
import the same class from a shared package.

Basic usage:

```typescript
import { LanceServerClient } from "./client";

const lance = new LanceServerClient("http://127.0.0.1:8080/v1");

console.log(await lance.ping());

await lance.createTable("documents", 4);
await lance.insert("documents", [
  {
    id: "doc-1",
    text: "first document",
    vector: [1, 0, 0, 0],
  },
]);

const matches = await lance.search("documents", [1, 0, 0, 0], 5);
console.log(matches);
```

Node.js 18 or newer provides the global `fetch` used by the example client.

## Build the executable again

You only need this section when the Rust source code changes. Building is not required
just to run the existing executable.

From the repository root:

```powershell
Set-Location "E:\lance-db\lancedb"
$env:PROTOC = "E:\lance-db\protoc\bin\protoc.exe"
cargo build --release -p lancedb-server
```

The optimized executable is written to:

```text
E:\lance-db\lancedb\target\release\lancedb-server.exe
```

## Configuration

| Environment variable | Default | Purpose |
| --- | --- | --- |
| `LANCEDB_DATA_DIR` | `./data` | LanceDB data directory |
| `LANCEDB_BIND` | `127.0.0.1:8080` | HTTP bind address and port |

Set the environment variables in the same terminal immediately before starting the
executable. PowerShell environment variables are not automatically shared with other
already-open terminals.

## Troubleshooting

### Connection refused

- Confirm terminal 1 still shows the running server.
- Confirm the client uses the same address and port as `LANCEDB_BIND`.
- Check whether another application already uses port `8080`.`

To use another port:

```powershell
$env:LANCEDB_BIND = "127.0.0.1:18080"
```

Then change `$baseUrl` to `http://127.0.0.1:18080/v1`.

### `VECTOR_DIMENSION_MISMATCH`

The number of values in every inserted or searched vector must equal the table's
`vector_dimension`.

### Table already exists

Either complete Step 14 to drop the existing test table or use a different table
name.

### Data directory errors

Ensure `LANCEDB_DATA_DIR` points to a directory the current Windows user can write to.

## Current security boundary

This server is intended for localhost or a trusted private environment. It currently
has no authentication or TLS. Do not bind it to a public network interface until those
controls are implemented.
