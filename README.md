# Apollo MCP Server for DoControl

This is a thin wrapper around [Apollo MCP Server](https://github.com/apollographql/apollo-mcp-server) configured specifically for DoControl's authentication flow.

## What is Apollo MCP Server?

Apollo MCP Server is a [Model Context Protocol](https://modelcontextprotocol.io/) server that exposes GraphQL operations as MCP tools. It provides a standard way for AI models to access and orchestrate GraphQL APIs.

For full documentation about Apollo MCP Server capabilities, see the [official documentation](https://www.apollographql.com/docs/apollo-mcp-server/).

## DoControl Authentication Flow

This wrapper handles DoControl's OAuth token refresh flow automatically:

### How It Works

1. **Token Refresh**: Uses `APOLLO_REFRESH_TOKEN` to obtain fresh access tokens from `APOLLO_REFRESH_URL`
2. **Auto-Refresh**: Tokens are automatically refreshed before expiration (5 minutes before)
3. **Config Update**: Fresh tokens are written back to the config file's auth section
4. **Background Task**: A background task continuously monitors and refreshes tokens
5. **GraphQL Requests**: All operations use the current valid access token

### Environment Variables

The server requires these environment variables:

```bash
# DoControl Token Refresh
APOLLO_REFRESH_TOKEN="your-refresh-token-from-docontrol"
APOLLO_REFRESH_URL="https://auth.prod.docontrol.io/refresh"
APOLLO_GRAPHQL_ENDPOINT="https://apollo-gateway-v4-api.prod.docontrol.io/graphql"

# Apollo GraphOS API Key
APOLLO_KEY="service:docontrol-api:your-apollo-key"

# Optional: Override the hardcoded graph ref (defaults to "docontrol-api@current")
# APOLLO_GRAPH_REF="docontrol-api@current"
```

### Configuration File

Create a YAML configuration file (e.g., `config.yaml`):

```yaml
# GraphQL endpoint
endpoint: "https://apollo-gateway-v4-api.prod.docontrol.io/graphql"

# Apollo GraphOS configuration
graphos:
  apollo_key: "${APOLLO_KEY}"  # From environment variable

# Use introspection to discover operations
operations: introspect

# Enable introspection
introspection:
  query: true
  mutation: true
```

**Note**: The `apollo_graph_ref` is hardcoded in the source code as `docontrol-api@current`. You can override it by setting the `APOLLO_GRAPH_REF` environment variable if needed.

**Note**: The `auth` section in the config file is automatically managed by the token refresh system. You don't need to manually specify it.

**Introspection Mode**: The server will introspect the GraphQL schema and automatically expose all queries and mutations as MCP tools. No need to manually define operations!

## MCP Client Configuration

### Cursor/Claude Desktop

Add this to your MCP configuration file:
- **Cursor**: `~/.cursor/mcp.json`
- **Claude Desktop**: `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS)

```json
{
  "mcpServers": {
    "docontrol": {
      "command": "/path/to/apollo-mcp-server",
      "args": ["/path/to/config.yaml"],
      "env": {
        "APOLLO_REFRESH_TOKEN": "your-refresh-token",
        "APOLLO_REFRESH_URL": "https://auth.prod.docontrol.io/refresh",
        "APOLLO_GRAPHQL_ENDPOINT": "https://apollo-gateway-v4-api.prod.docontrol.io/graphql",
        "APOLLO_GRAPH_REF": "docontrol-api@current",
        "APOLLO_KEY": "service:docontrol-api:your-key"
      }
    }
  }
}
```

### Using with MCP Inspector

For testing and debugging:

```bash
export APOLLO_REFRESH_TOKEN="your-refresh-token"
export APOLLO_REFRESH_URL="https://auth.prod.docontrol.io/refresh"
export APOLLO_GRAPHQL_ENDPOINT="https://apollo-gateway-v4-api.prod.docontrol.io/graphql"
export APOLLO_GRAPH_REF="docontrol-api@current"
export APOLLO_KEY="service:docontrol-api:your-key"

npx @modelcontextprotocol/inspector apollo-mcp-server config.yaml
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

Download the latest release for your platform from the [releases page](https://github.com/yourusername/dc-mcp-server/releases):
- **Linux**: `apollo-mcp-server-linux-x86_64.tar.gz`
- **macOS**: `apollo-mcp-server-macos-aarch64.tar.gz`

### From Source

```bash
cargo build --release
cp target/release/apollo-mcp-server /usr/local/bin/
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
- ✅ **Use environment variables** instead of hardcoding in config files
- ✅ **Store tokens securely** - use secret management systems (e.g., 1Password, AWS Secrets Manager)
- ✅ **Rotate tokens regularly** - follow DoControl security best practices
- ✅ **Limit permissions** - use read-only tokens when possible

**Secrets to protect:**
- `APOLLO_REFRESH_TOKEN` - DoControl OAuth refresh token
- `APOLLO_KEY` - Apollo Studio API key
- Config files containing tokens

## How Token Refresh Works

1. **Server Startup**: Reads `APOLLO_REFRESH_TOKEN` from environment
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
RUST_LOG=debug apollo-mcp-server config.yaml
```

## Upstream

This project is based on [Apollo MCP Server](https://github.com/apollographql/apollo-mcp-server). For general MCP server features and documentation, refer to the upstream project.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
