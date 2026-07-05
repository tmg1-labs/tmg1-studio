// リリース CI 用: タグ由来の version を各ファイルへ同期するスクリプト。
//
// sed/jq の OS 差（macOS/Linux/Windows ランナー）を避けるため Node で portable に処理する。
// 対象: package.json / src-tauri/tauri.conf.json / src-tauri/Cargo.toml。
// 使い方: node scripts/sync-version.mjs 0.2.0   （引数は "v" を除いた semver）

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const version = process.argv[2];

// 数値 3 連（プレリリース/ビルドメタ許容）の semver でなければ拒否する。
if (!version || !/^\d+\.\d+\.\d+/.test(version)) {
  console.error(`sync-version: invalid version argument: ${JSON.stringify(version)}`);
  console.error("usage: node scripts/sync-version.mjs <semver>  (e.g. 0.2.0)");
  process.exit(1);
}

// scripts/ の 1 つ上をリポジトリルートとして解決（呼び出し時の CWD に依存しない）。
const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

// JSON ファイルの version フィールドを差し替える（2 スペース整形・末尾改行）。
function patchJson(relPath) {
  const path = join(repoRoot, relPath);
  const obj = JSON.parse(readFileSync(path, "utf8"));
  obj.version = version;
  writeFileSync(path, JSON.stringify(obj, null, 2) + "\n");
  console.log(`sync-version: ${relPath} -> ${version}`);
}

patchJson("package.json");
patchJson("src-tauri/tauri.conf.json");

// Cargo.toml は先頭 [package] 直下の version 行を置換する（最初の version = "..." 行が対象）。
{
  const relPath = "src-tauri/Cargo.toml";
  const path = join(repoRoot, relPath);
  const text = readFileSync(path, "utf8");
  const replaced = text.replace(/^version = ".*"$/m, `version = "${version}"`);
  if (replaced === text) {
    console.error(`sync-version: no 'version = "..."' line found in ${relPath}`);
    process.exit(1);
  }
  writeFileSync(path, replaced);
  console.log(`sync-version: ${relPath} -> ${version}`);
}
