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

// DatastoreTools defines the MCP tools exposed by the Datastore proxy (native TCP).
var DatastoreTools = []ToolDef{
	{
		Name:        "datastore_query",
		Description: "Execute a ClickHouse SQL query via native protocol and return results as JSON rows",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"sql": map[string]string{"type": "string", "description": "ClickHouse SQL query"},
			},
			"required": []string{"sql"},
		},
	},
	{
		Name:        "datastore_exec",
		Description: "Execute a DDL or non-SELECT ClickHouse statement (CREATE, ALTER, DROP, etc.)",
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
		Name:        "datastore_insert",
		Description: "Bulk insert rows into a ClickHouse table via native batch protocol",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"table":    map[string]string{"type": "string", "description": "Target table name"},
				"database": map[string]string{"type": "string", "description": "Database name"},
				"columns":  map[string]interface{}{"type": "array", "items": map[string]string{"type": "string"}, "description": "Column names (inferred from rows if omitted)"},
				"rows": map[string]interface{}{
					"type":        "array",
					"description": "Array of row objects to insert",
				},
			},
			"required": []string{"table", "rows"},
		},
	},
	{
		Name:        "datastore_tables",
		Description: "List tables and their metadata in a ClickHouse database",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"database": map[string]string{"type": "string", "description": "Database name (default: configured database)"},
			},
		},
	},
	{
		Name:        "datastore_health",
		Description: "Check ClickHouse native TCP connection health and server version",
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

// DocumentDBTools defines the MCP tools exposed by the DocumentDB proxy.
var DocumentDBTools = []ToolDef{
	{
		Name:        "documentdb_find",
		Description: "Find documents in a collection matching a filter",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"collection": map[string]string{"type": "string", "description": "Collection name"},
				"filter":     map[string]string{"type": "object", "description": "MongoDB query filter"},
				"limit":      map[string]string{"type": "integer", "description": "Max documents to return"},
				"database":   map[string]string{"type": "string", "description": "Database name (default: hanzo)"},
			},
			"required": []string{"collection"},
		},
	},
	{
		Name:        "documentdb_insert",
		Description: "Insert documents into a collection",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"collection": map[string]string{"type": "string", "description": "Collection name"},
				"documents": map[string]interface{}{
					"type":        "array",
					"description": "Array of documents to insert",
				},
				"database": map[string]string{"type": "string", "description": "Database name"},
			},
			"required": []string{"collection", "documents"},
		},
	},
	{
		Name:        "documentdb_update",
		Description: "Update documents matching a filter",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"collection": map[string]string{"type": "string", "description": "Collection name"},
				"filter":     map[string]string{"type": "object", "description": "Match filter"},
				"update":     map[string]string{"type": "object", "description": "Update operations"},
				"database":   map[string]string{"type": "string", "description": "Database name"},
			},
			"required": []string{"collection", "filter", "update"},
		},
	},
	{
		Name:        "documentdb_delete",
		Description: "Delete documents matching a filter",
		Schema: map[string]interface{}{
			"type": "object",
			"properties": map[string]interface{}{
				"collection": map[string]string{"type": "string", "description": "Collection name"},
				"filter":     map[string]string{"type": "object", "description": "Match filter"},
				"database":   map[string]string{"type": "string", "description": "Database name"},
			},
			"required": []string{"collection", "filter"},
		},
	},
	{
		Name:        "documentdb_health",
		Description: "Check DocumentDB/FerretDB connection health",
		Schema: map[string]interface{}{
			"type": "object", "properties": map[string]interface{}{},
		},
	},
}

// DocumentDBResources defines MCP resources for the DocumentDB proxy.
var DocumentDBResources = []ResourceDef{
	{
		URI:         "hanzo://documentdb/collections",
		Name:        "DocumentDB Collections",
		Description: "List of collections and their indexes",
		MimeType:    "application/json",
	},
}
