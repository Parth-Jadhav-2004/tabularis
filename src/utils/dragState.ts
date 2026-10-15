let _table: string | null = null;
let _schema: string | null = null;
let _scope: { connectionId: string; database?: string } | null = null;

export const dragState = {
  get table() { return _table; },
  get schema() { return _schema; },
  get context() { return _table && _scope ? { name: _table, connectionId: _scope.connectionId, database: _scope.database ?? null, schema: _schema } : null; },
  start(name: string, schema?: string | null, scope?: { connectionId: string; database?: string }) {
    _table = name;
    _schema = schema || null;
    _scope = scope ?? null;
  },
  clear() {
    _table = null;
    _schema = null;
    _scope = null;
  },
};
