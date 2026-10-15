import { describe, expect, it } from "vitest";
import { activeAiMention, aiContextCatalogTables, aiContextDroppedTable, filterAiMentionTables, removeAiMention, type AiContextCatalogData } from "../../src/utils/aiContextMentions";
import type { SchemaData } from "../../src/contexts/DatabaseContext";
import { dragState } from "../../src/utils/dragState";

const scope = { connection_id: "connection", database: "app", schema: "public" };
function schema(...names: string[]): SchemaData {
  return { tables: names.map((name) => ({ name })), views: [], routines: [], triggers: [], isLoading: false, isLoaded: true };
}
function data(): AiContextCatalogData {
  return { databaseName: "app", tables: [{ name: "fallback" }], activeSchema: "public", selectedDatabases: [],
    capabilities: { schemas: true, sql_dialect: "postgres", views: true, routines: false, file_based: false, folder_based: false, identifier_quote: '"', alter_primary_key: false },
    schemaDataMap: { public: schema("users", "orders", "users"), other: schema("private") },
    databaseDataMap: {}, nestedDatabaseDataMap: {},
  };
}

describe("AI table mentions", () => {
  it("detects only the active mention at a collapsed caret and preserves surrounding text", () => {
    const text = "Explain @us next";
    const range = activeAiMention(text, 11)!;
    expect(range).toEqual({ start: 8, end: 11, search: "us" });
    expect(removeAiMention(text, range)).toBe("Explain  next");
    expect(activeAiMention("@用户", 3)?.search).toBe("用户");
    expect(activeAiMention("foo@example.com", 15)).toBeNull();
    expect(activeAiMention("@@users", 7)).toBeNull();
    expect(activeAiMention("@users", 3, 6)).toBeNull();
    expect(activeAiMention("@" + "x".repeat(81), 82)).toBeNull();
    expect(activeAiMention("@users", -1)).toBeNull();
  });
  it("filters Unicode names without duplicates and bounds each menu", () => {
    expect(filterAiMentionTables(["USERS", "users_archive", "users_archive", "other"], "Users")).toEqual(["USERS", "users_archive"]);
    expect(filterAiMentionTables(["用户", "订单"], "用")).toEqual(["用户"]);
    expect(filterAiMentionTables(Array.from({ length: 500 }, (_, i) => `table${i}`), "")).toHaveLength(30);
  });
  it("uses only the active schema cache and does not fall back across scopes", () => {
    const catalog = data();
    expect(aiContextCatalogTables(catalog, scope)).toEqual(["orders", "users"]);
    expect(aiContextCatalogTables(catalog, { ...scope, schema: null })).toEqual(["orders", "users"]);
    expect(aiContextCatalogTables(catalog, { ...scope, schema: "missing" })).toEqual([]);
    expect(aiContextCatalogTables(catalog, { ...scope, database: "other" })).toEqual([]);
    expect(aiContextCatalogTables(undefined, scope)).toEqual([]);
    expect(aiContextCatalogTables(catalog, null)).toEqual([]);
  });
  it("keeps nested databases and flat MySQL database caches separate", () => {
    const catalog = data();
    catalog.nestedDatabaseDataMap.analytics = { schemas: ["public"], schemasLoaded: true, isLoadingSchemas: false, selectedSchemas: ["public"], activeSchema: "public", needsSchemaSelection: false, schemaDataMap: { public: schema("events") } };
    expect(aiContextCatalogTables(catalog, { ...scope, database: "analytics" })).toEqual(["events"]);
    catalog.capabilities = { ...catalog.capabilities!, schemas: false, sql_dialect: "mysql" };
    catalog.selectedDatabases = ["app", "analytics"];
    catalog.databaseDataMap = { app: schema("users"), analytics: schema("events") };
    expect(aiContextCatalogTables(catalog, { ...scope, schema: "analytics" })).toEqual(["events"]);
    expect(aiContextCatalogTables(catalog, { ...scope, schema: "missing" })).toEqual([]);
  });
  it("accepts only catalog-backed drops from the same connection, database and schema", () => {
    const catalog = data();
    const drag = { name: "users", connectionId: "connection", database: "app", schema: "public" };
    expect(aiContextDroppedTable(drag, catalog, scope)).toBe("users");
    for (const wrong of [{ connectionId: "other" }, { database: "other" }, { schema: "other" }, { name: "unknown" }]) {
      expect(aiContextDroppedTable({ ...drag, ...wrong }, catalog, scope)).toBeNull();
    }
    expect(aiContextDroppedTable(null, catalog, scope)).toBeNull();
    catalog.capabilities = { ...catalog.capabilities!, schemas: false, sql_dialect: "mysql" };
    catalog.selectedDatabases = ["app", "analytics"];
    catalog.databaseDataMap.analytics = schema("events");
    expect(aiContextDroppedTable({ ...drag, name: "events", database: null, schema: "analytics" }, catalog, { ...scope, schema: "analytics" })).toBe("events");
  });
  it("preserves legacy explorer drag state and clears its new scope together", () => {
    dragState.start("users", "public");
    expect(dragState.table).toBe("users");
    expect(dragState.context).toBeNull();
    dragState.start("users", "public", { connectionId: "connection", database: "app" });
    expect(dragState.context).toEqual({ name: "users", schema: "public", connectionId: "connection", database: "app" });
    dragState.clear();
    expect(dragState.context).toBeNull();
    expect(dragState.table).toBeNull();
    expect(dragState.schema).toBeNull();
  });
});
