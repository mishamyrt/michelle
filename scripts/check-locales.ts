import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const english = Bun.YAML.parse(
  readFileSync(new URL("../locales/app.yml", import.meta.url), "utf8"),
) as Record<string, { en: string }>;
const russian = Bun.YAML.parse(
  readFileSync(new URL("../locales/ru.yml", import.meta.url), "utf8"),
) as Record<string, string>;

assert.deepEqual(Object.keys(russian).sort(), Object.keys(english).sort());
const placeholders = (text: string) =>
  [...text.matchAll(/%\{\w+\}/g)].map(([match]) => match).sort();
for (const [key, value] of Object.entries(english)) {
  if (key === "_version") continue;
  assert.equal(typeof russian[key], "string", key);
  assert.ok(russian[key].trim(), key);
  assert.deepEqual(placeholders(russian[key]), placeholders(value.en), key);
}
console.log(`Validated ${Object.keys(english).length - 1} Russian translations`);
