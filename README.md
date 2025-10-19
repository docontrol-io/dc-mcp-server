# DoControl MCP Server

This is a thin wrapper around [Apollo MCP Server](https://github.com/apollographql/apollo-mcp-server) configured specifically for DoControl's authentication flow.

## What is Apollo MCP Server?

Apollo MCP Server is a [Model Context Protocol](https://modelcontextprotocol.io/) server that exposes GraphQL operations as MCP tools. It provides a standard way for AI models to access and orchestrate GraphQL APIs.

For full documentation about Apollo MCP Server capabilities, see the [official documentation](https://www.apollographql.com/docs/apollo-mcp-server/).

## DoControl Authentication Flow

This wrapper handles DoControl's OAuth token refresh flow automatically:

### How It Works

1. **Token Refresh**: Uses `DC_REFRESH_TOKEN` to obtain fresh access tokens from `DC_REFRESH_URL`
2. **Auto-Refresh**: Tokens are automatically refreshed before expiration (5 minutes before)
3. **Config Update**: Fresh tokens are written back to the config file's auth section
4. **Background Task**: A background task continuously monitors and refreshes tokens
5. **GraphQL Requests**: All operations use the current valid access token

### Environment Variables

**Required environment variables** (must be in MCP client config):

```bash
# DoControl Token Refresh (Required)
DC_TOKEN_REFRESH_ENABLED="true"
DC_REFRESH_TOKEN="your-refresh-token-from-docontrol"
DC_REFRESH_URL="https://auth.prod.docontrol.io/refresh"
```

**Optional environment variables** (recommended to move to config file):

```bash
# GraphQL Endpoint (OPTIONAL - recommended to put in config.yaml as 'endpoint')
DC_GRAPHQL_ENDPOINT="https://apollo-gateway-v4-api.prod.docontrol.io/graphql"

# Apollo GraphOS API Key (OPTIONAL - recommended to put in config.yaml)
DC_API_KEY="service:docontrol-api:your-apollo-key"

# Graph Reference Override (OPTIONAL - defaults to "docontrol-api@current")
# DC_GRAPH_REF="docontrol-api@current"
```

**💡 Recommendation**: Move `DC_GRAPHQL_ENDPOINT` and `DC_API_KEY` to your config file (see below) to keep all non-sensitive configuration in one place.

### Configuration File

Create a YAML configuration file (e.g., `config.yaml`):

```yaml
# GraphQL endpoint URL
endpoint: https://apollo-gateway-v4-api.prod.docontrol.io/graphql

# Transport configuration (stdio for MCP)
transport:
  type: stdio

# Authentication headers (automatically managed by token refresh)
headers:
  Authorization: Bearer <token-will-be-auto-refreshed>

# Apollo GraphOS integration
graphos:
  apollo-graph-ref: docontrol-api@current
  apollo-key: service:docontrol-api:your-apollo-key

# Mutation mode: "none" (read-only), "all" (full access)
allow-mutations: none

# Operation source: use introspection to discover operations
operations:
  - introspect

# Logging configuration
logging:
  level: error
  format: plain
  color: false

# Introspection tools configuration
introspection:
  execute:
    enabled: true    # Enable execute tool to run queries
  introspect:
    enabled: true    # Enable introspect tool for schema discovery
    minify: true     # Minify schema output
  search:
    enabled: true    # Enable search tool for finding types
    minify: true     # Minify search results
    index_memory_bytes: 50000000  # Memory limit for search index
    leaf_depth: 1    # Depth for leaf type expansion
  validate:
    enabled: true    # Enable validate tool for query validation
```

#### Configuration Options Explained

**Core Settings:**
- `endpoint`: The DoControl GraphQL API endpoint
- `transport.type`: `stdio` for MCP communication (required for MCP clients)

**Authentication:**
- `headers.Authorization`: Automatically updated by token refresh system
- The token in this section is managed by the server - it will be overwritten on startup and during refresh

**GraphOS Integration:**
- `apollo-graph-ref`: Your graph reference in Apollo Studio (e.g., `docontrol-api@current`)
- `apollo-key`: Your Apollo Studio API key for schema registry access

**Security:**
- `allow-mutations`: Set to `none` for read-only access, `all` to allow mutations

**Operations:**
- `introspect`: Use introspection to discover all queries and mutations automatically
- Alternative: `uplink` to use Apollo Studio operation collections

**Introspection Tools:**
The server provides 4 MCP tools when introspection is enabled:

1. **`execute`**: Run GraphQL queries and mutations
   - Validates operation syntax
   - Executes against the live endpoint
   - Returns JSON results

2. **`introspect`**: Explore the GraphQL schema
   - Get type information with hierarchy
   - Discover fields, arguments, and descriptions
   - Navigate relationships between types

3. **`search`**: Find types in the schema by name
   - Fuzzy search across all types
   - Returns matching type definitions
   - Useful for discovery

4. **`validate`**: Validate GraphQL operations before execution
   - Syntax checking
   - Schema validation
   - Helpful for debugging

**Note**: The `apollo_key` can reference environment variables using `${DC_API_KEY}` syntax, but it's recommended to put the actual value directly in the config file.

**Note**: The `Authorization` header is automatically managed by the token refresh system. You don't need to manually update it.

## Environment Variable vs Config File

### What Goes Where?

| Setting | Environment Variable | Config File | Recommendation |
|---------|---------------------|-------------|----------------|
| **Refresh Token** | `DC_REFRESH_TOKEN` | ❌ Not supported | ✅ Keep in env (security) |
| **Refresh URL** | `DC_REFRESH_URL` | ❌ Not supported | ✅ Keep in env |
| **GraphQL Endpoint** | `DC_GRAPHQL_ENDPOINT` | `endpoint` | ✅ **Move to config** (cleaner) |
| **Apollo API Key** | `DC_API_KEY` | `graphos.apollo-key` | ✅ **Move to config** (cleaner) |
| **Graph Ref** | `DC_GRAPH_REF` | `graphos.apollo-graph-ref` | ⚠️ Either (defaults to hardcoded value) |

### Recommended Setup

**Minimal MCP Client Config** (`~/.cursor/mcp.json`):
```json
{
  "mcpServers": {
    "docontrol-mcp-server": {
      "command": "/usr/local/bin/dc-mcp-server",
      "args": ["/path/to/config.yaml"],
      "env": {
        "DC_TOKEN_REFRESH_ENABLED": "true",
        "DC_REFRESH_TOKEN": "YOUR_REFRESH_TOKEN",
        "DC_REFRESH_URL": "https://auth.prod.docontrol.io/refresh",
        "RUST_LOG": "info",
        "RUSTLS_SYSTEM_CERT_ROOT": "1"
      }
    }
  }
}
```

**Complete Config File** (`config.yaml`):
```yaml
endpoint: https://apollo-gateway-v4-api.prod.docontrol.io/graphql
transport:
  type: stdio
headers:
  Authorization: Bearer placeholder  # Auto-updated
graphos:
  apollo-graph-ref: docontrol-api@current
  apollo-key: service:docontrol-api:YOUR_APOLLO_KEY  # ← Put DC_API_KEY here
allow-mutations: none
operations:
  - introspect
```

**Benefits of this approach**:
- ✅ Sensitive tokens stay in environment variables (more secure)
- ✅ Non-sensitive config (API keys, endpoints) in config file (easier to manage)
- ✅ Config file can be version controlled (without secrets)
- ✅ Easier to update non-sensitive settings without restarting MCP client
- ✅ No duplication - `endpoint` and `DC_GRAPHQL_ENDPOINT` are the same, so just use config file

## Quick Start Setup Guide

### Step 1: Install the Server

**Option A: Download from Releases**
```bash
# macOS (Apple Silicon)
curl -L https://github.com/docontrol-io/dc-mcp-server/releases/latest/download/dc-mcp-server-macos-aarch64.tar.gz | tar xz
chmod +x dc-mcp-server

# Linux
curl -L https://github.com/docontrol-io/dc-mcp-server/releases/latest/download/dc-mcp-server-linux-x86_64.tar.gz | tar xz
chmod +x dc-mcp-server

# Move to a permanent location
sudo mv dc-mcp-server /usr/local/bin/
```

**Option B: Build from Source**
```bash
git clone https://github.com/docontrol-io/dc-mcp-server.git
cd dc-mcp-server
cargo build --release
sudo cp target/release/dc-mcp-server /usr/local/bin/
```

### Step 2: Create Configuration File

Create a file named `docontrol-config.yaml`:

```yaml
endpoint: https://apollo-gateway-v4-api.prod.docontrol.io/graphql
transport:
  type: stdio
headers:
  Authorization: Bearer placeholder  # Will be auto-updated
graphos:
  apollo-graph-ref: docontrol-api@current
  apollo-key: service:docontrol-api:YOUR_APOLLO_KEY_HERE
allow-mutations: none
operations:
  - introspect
logging:
  level: error
  format: plain
  color: false
introspection:
  execute:
    enabled: true
  introspect:
    enabled: true
    minify: true
  search:
    enabled: true
    minify: true
  validate:
    enabled: true
```

**Replace `YOUR_APOLLO_KEY_HERE`** with your actual Apollo Studio API key.

### Step 3: Get Your Credentials

You'll need two secrets from DoControl:

1. **Refresh Token** (`DC_REFRESH_TOKEN`):
   - Obtain from DoControl OAuth authentication flow
   - This is a long-lived token used to get fresh access tokens
   - Keep this secret secure!

2. **Apollo API Key** (`DC_API_KEY`):
   - Format: `service:docontrol-api:xxxxx`
   - Used to access Apollo Studio for schema registry

### Step 4: Configure Your MCP Client

**For Cursor:**

Edit `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "dc-mcp-server": {
      "command": "/usr/local/bin/dc-mcp-server",
      "args": ["/absolute/path/to/docontrol-config.yaml"],
      "env": {
        "DC_TOKEN_REFRESH_ENABLED": "true",
        "DC_REFRESH_TOKEN": "YOUR_REFRESH_TOKEN_HERE",
        "DC_REFRESH_URL": "https://auth.prod.docontrol.io/refresh",
        "RUST_LOG": "info",
        "RUSTLS_SYSTEM_CERT_ROOT": "1"
      }
    }
  }
}
```

**For Claude Desktop (macOS):**

Edit `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "dc-mcp-server": {
      "command": "/usr/local/bin/dc-mcp-server",
      "args": ["/absolute/path/to/docontrol-config.yaml"],
      "env": {
        "DC_TOKEN_REFRESH_ENABLED": "true",
        "DC_REFRESH_TOKEN": "YOUR_REFRESH_TOKEN_HERE",
        "DC_REFRESH_URL": "https://auth.prod.docontrol.io/refresh",
        "RUST_LOG": "info",
        "RUSTLS_SYSTEM_CERT_ROOT": "1"
      }
    }
  }
}
```

**Important Notes:**
- Use **absolute paths** for both the command and config file
- Replace `YOUR_REFRESH_TOKEN_HERE` with your actual refresh token
- **DO NOT** include `DC_API_KEY` or `DC_GRAPHQL_ENDPOINT` here - they should be in your config.yaml file
- The `RUSTLS_SYSTEM_CERT_ROOT=1` is required for SSL certificate validation

### Step 5: Restart Your MCP Client

- **Cursor**: Restart the application or reload the window
- **Claude Desktop**: Quit and restart the application

### Step 6: Verify Setup

In your MCP client, you should see 4 new tools available:
- ✅ `execute` - Run GraphQL queries
- ✅ `introspect` - Explore the schema
- ✅ `search` - Search for types
- ✅ `validate` - Validate queries

Try asking: "What is the company information?" or "Show me the company details"

## MCP Client Configuration Reference

### Using with MCP Inspector

For testing and debugging:

```bash
export DC_TOKEN_REFRESH_ENABLED="true"
export DC_REFRESH_TOKEN="your-refresh-token"
export DC_REFRESH_URL="https://auth.prod.docontrol.io/refresh"

# Note: DC_GRAPHQL_ENDPOINT and DC_API_KEY should be in your config.yaml file, not as env vars
npx @modelcontextprotocol/inspector dc-mcp-server config.yaml
```

## How Introspection Works

The server automatically discovers all available GraphQL operations by introspecting the schema:

1. **Schema Discovery**: Introspects the GraphQL endpoint to discover all types and fields
2. **Tool Generation**: Each query and mutation becomes an MCP tool
3. **Dynamic**: Changes to the GraphQL schema are automatically reflected
4. **No Manual Configuration**: No need to maintain operation files

All queries and mutations from the DoControl GraphQL API are automatically available as tools to AI models.

## Installation

### From Release

Download the latest release for your platform from the [releases page](https://github.com/docontrol-io/dc-mcp-server/releases):
- **Linux**: `dc-mcp-server-linux-x86_64.tar.gz`
- **macOS**: `dc-mcp-server-macos-aarch64.tar.gz`
- **Windows**: `dc-mcp-server-windows-x86_64.tar.gz`

### From Source

```bash
cargo build --release --package dc-mcp-server
cp target/release/dc-mcp-server /usr/local/bin/
```

## Example Setup

1. **Create configuration file** (`config.yaml`):
```yaml
endpoint: "https://apollo-gateway-v4-api.prod.docontrol.io/graphql"
operations: introspect
introspection:
  query: true
  mutation: true
```

2. **Get your credentials**:
   - **Refresh Token**: From DoControl OAuth flow (secret)
   - **Apollo Graph Ref**: Your graph identifier, e.g., `docontrol-api@current` (internal)
   - **Apollo Key**: API key from Apollo Studio (secret)

3. **Configure your MCP client** with environment variables (see configuration examples above)

4. **Start your MCP client** - the server handles all authentication and operation discovery automatically!

The AI assistant will have access to all GraphQL queries and mutations from the DoControl API.

## Security Best Practices

**All credentials are secrets and should be protected:**

- ✅ **Never commit credentials** to version control
- ✅ **Use environment variables for sensitive tokens** (`DC_REFRESH_TOKEN`)
- ✅ **Store API keys in config files** (`graphos.apollo-key`) - easier to manage, can be encrypted at rest
- ✅ **Store tokens securely** - use secret management systems (e.g., 1Password, AWS Secrets Manager)
- ✅ **Rotate tokens regularly** - follow DoControl security best practices
- ✅ **Limit permissions** - use read-only tokens when possible (set `allow-mutations: none`)

**Secrets to protect:**
- `DC_REFRESH_TOKEN` - DoControl OAuth refresh token (keep in environment variables)
- `graphos.apollo-key` - Apollo Studio API key (can be in config file with proper file permissions)
- Config files containing tokens (set appropriate file permissions: `chmod 600 config.yaml`)

## How Token Refresh Works

1. **Server Startup**: Reads `DC_REFRESH_TOKEN` from environment
2. **Initial Refresh**: Immediately refreshes to get a valid access token
3. **Config Update**: Writes access token to config file's `auth` section
4. **Token Verification**: Verifies token works with a test GraphQL request
5. **Background Task**: Monitors token expiration and refreshes 5 minutes before expiry
6. **Automatic Updates**: Config file is automatically updated with new tokens

## Development

### Running Tests

```bash
cargo test --workspace
```

### Building

```bash
cargo build --release
```

### Debugging

Enable debug logging:
```bash
RUST_LOG=debug dc-mcp-server config.yaml
```

## Upstream

This project is based on [Apollo MCP Server](https://github.com/apollographql/apollo-mcp-server). For general MCP server features and documentation, refer to the upstream project.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
