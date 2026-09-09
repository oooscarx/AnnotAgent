// Offline preparation for an explicitly authorized isolated LIVE recording.
// Does not copy projects, conversations, annotations or execution history.
import { readFileSync, writeFileSync, realpathSync } from 'node:fs';
import { join, basename } from 'node:path';
import assert from 'node:assert/strict';
const [sourceArg, destinationArg] = process.argv.slice(2);
const source = realpathSync(sourceArg), destination = realpathSync(destinationArg);
assert.notEqual(source, destination);
assert.match(destination, /^\/private\/tmp\/AnnotAgent-LIVE-recording-/);
assert.match(basename(destination), /^AnnotAgent-LIVE-recording-/);
function registry(relative) {
  const value = JSON.parse(readFileSync(join(source, '.annotagent', relative), 'utf8'));
  value.events = [];
  value.references = Array.isArray(value.references) ? [] : {};
  // Installation/license receipts describe existing installed models, not new permission.
  const rewrite = v => typeof v === 'string' ? v.replaceAll(source, destination)
    : Array.isArray(v) ? v.map(rewrite)
    : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k,x])=>[k,rewrite(x)])) : v;
  writeFileSync(join(destination, '.annotagent', relative), JSON.stringify(rewrite(value), null, 2)+'\n', {mode:0o600});
}
registry('plugins/plugin-registry.json');
registry('model-bundle-registry.json');
writeFileSync(join(destination,'LIVE_RECORDING.json'),JSON.stringify({purpose:'Authorized isolated LIVE product recording',synthetic_model:false,source_history_copied:false,created_at:new Date().toISOString()},null,2)+'\n',{mode:0o600});
console.log('Prepared isolated installation registry; no secrets printed.');
