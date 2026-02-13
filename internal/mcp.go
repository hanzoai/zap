// Package internal provides shared MCP-compatible tool/resource definitions
// for all ZAP proxy backends.
//
// ZAP's schema natively maps 1:1 with MCP (Model Context Protocol):
//   - ZAP tools  → MCP tools (listTools, callTool)
//   - ZAP resources → MCP resources (listResources, readResource)
//   - ZAP prompts → MCP prompts (listPrompts, getPrompt)
//
// Any service implementing the ZAP Zap interface gets MCP for free via
// the ZAP Gateway (zapd), which bridges ZAP ↔ MCP transports
// (stdio, HTTP/SSE, WebSocket).
//
// Schema reference: zap/schema/zap.capnp
package internal

// ToolDef defines a ZAP/MCP tool exposed by a proxy backend.
type ToolDef struct {
	Name        string                 `json:"name"`
	Description string                 `json:"description"`
	Schema      map[string]interface{} `json:"inputSchema"`
}

// ResourceDef defines a ZAP/MCP resource exposed by a proxy backend.
type ResourceDef struct {
	URI         string `json:"uri"`
	Name        string `json:"name"`
	Description string `json:"description"`
	MimeType    string `json:"mimeType"`
}

// SQLTools defines the MCP tools exposed by the SQL proxy.
var SQLTools = []ToolDef{
	{
		Name:        "sql_query",
		Description: "Execute a read-only SQL query against PostgreSQL and return results as JSON rows",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"sql":  map[string]string{"type": "string", "description": "SQL SELECT query"},
				"args": map[string]string{"type": "array", "description": "Query parameters"},
			},
			"required": []string{"sql"},
		},
	},
	{
		Name:        "sql_exec",
		Description: "Execute a write SQL statement (INSERT, UPDATE, DELETE) and return affected row count",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"sql":  map[string]string{"type": "string", "description": "SQL statement"},
				"args": map[string]string{"type": "array", "description": "Statement parameters"},
			},
			"required": []string{"sql"},
		},
	},
	{
		Name:        "sql_health",
		Description: "Check PostgreSQL connection health",
		Schema: map[string]interface{}{
			"type": "object", "properties": map[string]interface{}{},
		},
	},
}

// KVTools defines the MCP tools exposed by the KV proxy.
var KVTools = []ToolDef{
	{
		Name:        "kv_get",
		Description: "Get a value by key from Hanzo KV (Valkey/Redis)",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"key": map[string]string{"type": "string", "description": "Key to retrieve"},
			},
			"required": []string{"key"},
		},
	},
	{
		Name:        "kv_set",
		Description: "Set a key-value pair in Hanzo KV",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"key":   map[string]string{"type": "string", "description": "Key to set"},
				"value": map[string]string{"type": "string", "description": "Value to set"},
				"ttl":   map[string]string{"type": "integer", "description": "TTL in seconds (0 = no expiry)"},
			},
			"required": []string{"key", "value"},
		},
	},
	{
		Name:        "kv_mget",
		Description: "Get multiple values by keys from Hanzo KV",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"keys": map[string]interface{}{
					"type":  "array",
					"items": map[string]string{"type": "string"},
				},
			},
			"required": []string{"keys"},
		},
	},
	{
		Name:        "kv_cmd",
		Description: "Execute an arbitrary Valkey/Redis command",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"cmd":  map[string]string{"type": "string", "description": "Command name (e.g. HGET, LPUSH)"},
				"args": map[string]interface{}{"type": "array", "items": map[string]string{"type": "string"}},
			},
			"required": []string{"cmd"},
		},
	},
}

// DatastoreTools defines the MCP tools exposed by the Datastore proxy.
var DatastoreTools = []ToolDef{
	{
		Name:        "datastore_query",
		Description: "Execute a ClickHouse SQL query and return results as JSON rows",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"sql":      map[string]string{"type": "string", "description": "ClickHouse SQL query"},
				"database": map[string]string{"type": "string", "description": "Database name (default: console)"},
				"format":   map[string]string{"type": "string", "description": "Output format (default: JSONEachRow)"},
			},
			"required": []string{"sql"},
		},
	},
	{
		Name:        "datastore_insert",
		Description: "Bulk insert rows into a ClickHouse table (optimized for AI telemetry)",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"table":    map[string]string{"type": "string", "description": "Target table name"},
				"database": map[string]string{"type": "string", "description": "Database name"},
				"rows": map[string]interface{}{
					"type":        "array",
					"description": "Array of row objects to insert",
				},
			},
			"required": []string{"table", "rows"},
		},
	},
	{
		Name:        "datastore_health",
		Description: "Check ClickHouse connection health",
		Schema: map[string]interface{}{
			"type": "object", "properties": map[string]interface{}{},
		},
	},
}

// SQLResources defines MCP resources for the SQL proxy.
var SQLResources = []ResourceDef{
	{
		URI:         "hanzo://sql/schema",
		Name:        "Database Schema",
		Description: "PostgreSQL database schema (tables, columns, indexes)",
		MimeType:    "application/json",
	},
}

// KVResources defines MCP resources for the KV proxy.
var KVResources = []ResourceDef{
	{
		URI:         "hanzo://kv/info",
		Name:        "KV Server Info",
		Description: "Valkey/Redis server info and statistics",
		MimeType:    "text/plain",
	},
}

// DatastoreResources defines MCP resources for the Datastore proxy.
var DatastoreResources = []ResourceDef{
	{
		URI:         "hanzo://datastore/tables",
		Name:        "Datastore Tables",
		Description: "ClickHouse table definitions and schemas",
		MimeType:    "application/json",
	},
}
