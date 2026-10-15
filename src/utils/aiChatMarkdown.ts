import { splitStatements } from "./sql";

export function isAiSqlCode(language: string, code: string, dialect?: string): boolean {
  if (language) return /^(sql|postgresql|postgres|mysql|sqlite|tsql|plsql)$/i.test(language);
  // Conservative best effort for untagged fences; never classify prose/inline code.
  const statements = splitStatements(code, dialect);
  return statements.length > 0 && statements.every(({ text }) =>
    /^\s*(?:(?:--[^\n]*\n|\/\*[\s\S]*?\*\/)\s*)*(?:select\s|with\s|insert\s+into\s|update\s+\S+\s+set\s|delete\s+from\s|create\s+(?:table|view|index)\s|alter\s+table\s|drop\s+(?:table|view|index)\s|explain\s)/i.test(text));
}
