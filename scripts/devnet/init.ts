// One-time Config initialization on devnet. Dry run by default; pass --yes to send.
// Usage: npm run devnet:init [-- --yes]

import { TOKEN_PROGRAM_ADDRESS } from '@solana-program/token';
import { address, getAddressDecoder, getAddressEncoder, getProgramDerivedAddress } from '@solana/kit';

import {
    ARENACHAIN_PROGRAM_ADDRESS,
    findConfigPda,
    findFeeVaultPda,
    getAdminInitializeConfigInstructionAsync,
} from '../../clients/js/src';
import { devnetClient, explorerTx, loadKeypair, readDeployment, updateDeployment, USDC, walletPath } from './lib';

const LOADER_V3 = address('BPFLoaderUpgradeab1e11111111111111111111111');
const PARAMS = {
    holdThreshold: 250n * USDC, // above demo pool sizes until the dispute freeze exists
    holdSecs: 24n * 3600n,
    resultWindowSecs: 2n * 3600n,
};

function fail(msg: string): never {
    console.error(`ABORT: ${msg}`);
    process.exit(1);
}

const wallet = await loadKeypair(walletPath());
const client = await devnetClient(wallet);
const dep = readDeployment();
const mint = address(dep.usdcMint);
if (dep.programId !== ARENACHAIN_PROGRAM_ADDRESS) fail('deployment programId differs from the client');

// Program deployed, upgrade authority = this wallet (only it may initialize).
const program = await client.rpc.getAccountInfo(ARENACHAIN_PROGRAM_ADDRESS, { encoding: 'base64' }).send();
if (!program.value?.executable || program.value.owner !== LOADER_V3) fail('program not deployed');
const [programData] = await getProgramDerivedAddress({
    programAddress: LOADER_V3,
    seeds: [getAddressEncoder().encode(ARENACHAIN_PROGRAM_ADDRESS)],
});
const pd = await client.rpc.getAccountInfo(programData, { encoding: 'base64', dataSlice: { offset: 0, length: 45 } }).send();
const pdBytes = Buffer.from(pd.value?.data[0] ?? '', 'base64');
if (pdBytes[12] !== 1) fail('program has no upgrade authority');
const upgradeAuthority = getAddressDecoder().decode(pdBytes.subarray(13, 45));
if (upgradeAuthority !== wallet.address) fail(`upgrade authority is ${upgradeAuthority}, not the wallet`);

// Mint: classic SPL Token, 6 decimals, no freeze authority (the program re-checks this).
const mintInfo = await client.rpc.getAccountInfo(mint, { encoding: 'base64' }).send();
if (mintInfo.value?.owner !== TOKEN_PROGRAM_ADDRESS) fail('mint is not a classic SPL Token mint');
const mintBytes = Buffer.from(mintInfo.value.data[0], 'base64');
if (mintBytes[44] !== 6) fail(`mint has ${mintBytes[44]} decimals`);
if (mintBytes.readUInt32LE(46) !== 0) fail('mint has a freeze authority');

// Not initialized yet.
const [config] = await findConfigPda();
const [feeVault] = await findFeeVaultPda();
if ((await client.rpc.getAccountInfo(config).send()).value) fail(`Config ${config} already exists`);

console.log(`Cluster:      devnet (genesis verified)
Program:      ${ARENACHAIN_PROGRAM_ADDRESS}
Payer/admin:  ${wallet.address}
Dispute auth: ${wallet.address}
Treasury:     ${wallet.address}
USDC mint:    ${mint}
Config PDA:   ${config}
Fee vault:    ${feeVault}
Params:       hold_threshold ${PARAMS.holdThreshold / USDC} USDC, hold ${PARAMS.holdSecs}s, result window ${PARAMS.resultWindowSecs}s
Cost:         ~0.0033 SOL (Config 227 B + fee vault 165 B rent, 1 fee)`);

if (!process.argv.includes('--yes')) {
    console.log('\nDry run. Re-run with --yes to send.');
    process.exit(0);
}

const ix = await getAdminInitializeConfigInstructionAsync({
    authority: wallet,
    usdcMint: mint,
    programData,
    admin: wallet.address,
    disputeAuthority: wallet.address,
    treasury: wallet.address,
    params: PARAMS,
});
const result = await client.sendTransaction(ix);
const signature = result.context.signature;
updateDeployment({ config, feeVault, configInitSignature: signature, configParams: {
    holdThreshold: PARAMS.holdThreshold.toString(),
    holdSecs: PARAMS.holdSecs.toString(),
    resultWindowSecs: PARAMS.resultWindowSecs.toString(),
} });
console.log(`\nInitialized. ${explorerTx(signature)}`);
