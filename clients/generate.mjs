// Regenerates the Kit client in clients/js/src/generated from the Anchor IDL.
// Run after every program change: `anchor build && npm run generate`.
import { readFileSync } from 'node:fs';
import { rootNodeFromAnchor } from '@codama/nodes-from-anchor';
import { renderVisitor } from '@codama/renderers-js';
import { createFromRoot } from 'codama';

const idl = JSON.parse(readFileSync(new URL('../target/idl/arenachain.json', import.meta.url), 'utf8'));
const codama = createFromRoot(rootNodeFromAnchor(idl));
await codama.accept(
    renderVisitor(new URL('./js', import.meta.url).pathname, {
        generatedFolder: 'src/generated',
        deleteFolderBeforeRendering: true,
        syncPackageJson: false,
    }),
);
console.log('Generated clients/js/src/generated');
