import validator from 'gltf-validator';
import { readFileSync } from 'fs';
for (const f of process.argv.slice(2)) {
  const r = await validator.validateBytes(new Uint8Array(readFileSync(f)), { maxIssues: 20 });
  const i = r.issues;
  console.log(`${f.split('/').pop()}: errors ${i.numErrors}, warnings ${i.numWarnings}, infos ${i.numInfos}; nodes ${r.info.nodeCount ?? '-'}, skins ${r.info.hasSkins}, morphs ${r.info.hasMorphTargets}`);
  for (const m of i.messages.filter(m => m.severity <= 1).slice(0, 8)) console.log(`   [${m.severity}] ${m.code} ${m.pointer ?? ''} ${m.message}`);
}
