// Regenerates the Kit client in clients/js/src/generated from the Anchor IDL.
// Run after every program change: `anchor build && npm run generate`.
import { readFileSync } from 'node:fs';
import { rootNodeFromAnchor } from '@codama/nodes-from-anchor';
import { renderVisitor } from '@codama/renderers-js';
import {
    accountValueNode,
    createFromRoot,
    deleteNodesVisitor,
    pdaLinkNode,
    pdaSeedValueNode,
    pdaValueNode,
    updateInstructionsVisitor,
} from 'codama';

const idl = JSON.parse(readFileSync(new URL('../target/idl/arenachain.json', import.meta.url), 'utf8'));
const codama = createFromRoot(rootNodeFromAnchor(idl));

// The IDL yields one PDA per instruction account, so the same PDA shows up under several names
// (creatorProfile, joinerProfile, serverStartMatchGameServer). Point those accounts at the
// shared `profile` / `gameServer` PDAs and drop the duplicates.
const profileOf = (account) =>
    pdaValueNode(pdaLinkNode('profile'), [pdaSeedValueNode('authority', accountValueNode(account))]);
codama.update(
    updateInstructionsVisitor({
        playerCreateMatch: { accounts: { creatorProfile: { defaultValue: profileOf('creator') } } },
        playerJoinMatch: { accounts: { joinerProfile: { defaultValue: profileOf('joiner') } } },
        serverStartMatch: {
            accounts: {
                gameServer: {
                    defaultValue: pdaValueNode(pdaLinkNode('gameServer'), [
                        pdaSeedValueNode('signerKey', accountValueNode('serverSigner')),
                    ]),
                },
            },
        },
    }),
);
codama.update(
    deleteNodesVisitor([
        '[pdaNode]creatorProfile',
        '[pdaNode]joinerProfile',
        '[pdaNode]serverStartMatchGameServer',
    ]),
);

await codama.accept(
    renderVisitor(new URL('./js', import.meta.url).pathname, {
        generatedFolder: 'src/generated',
        deleteFolderBeforeRendering: true,
        syncPackageJson: false,
    }),
);
console.log('Generated clients/js/src/generated');
