// End-to-end match on devnet: register the game server (once), two throwaway players with test
// USDC, create -> join -> start -> server-signed Win -> payout. The deploy wallet pays fees and
// relays the result; leftover player SOL is returned at the end.
// Usage: npm run devnet:e2e

import { getTransferSolInstruction } from '@solana-program/system';
import {
    fetchToken,
    findAssociatedTokenPda,
    getCreateAssociatedTokenIdempotentInstructionAsync,
    getMintToCheckedInstruction,
    TOKEN_PROGRAM_ADDRESS,
} from '@solana-program/token';
import {
    address,
    type Address,
    type KeyPairSigner,
    lamports,
    nonDivisibleSequentialInstructionPlan,
    signBytes,
} from '@solana/kit';

import {
    ARENACHAIN_PROGRAM_ADDRESS,
    fetchMatch,
    fetchMaybeGameServer,
    findArenaMatchPda,
    findFeeVaultPda,
    findGameServerPda,
    findProfilePda,
    findVaultPda,
    getAdminRegisterServerInstructionAsync,
    getEd25519VerifyInstruction,
    getPlayerCreateMatchInstructionAsync,
    getPlayerCreateProfileInstructionAsync,
    getPlayerJoinMatchInstructionAsync,
    getResultMessage,
    getServerStartMatchInstructionAsync,
    getSubmitResultInstructionAsync,
    MatchStatus,
    ResultKind,
} from '../../clients/js/src';
import {
    devnetClient,
    explorerAddress,
    explorerTx,
    failedBeforeSubmit,
    loadKeypair,
    newSavedKeypair,
    readDeployment,
    retry,
    savedE2eKeypairs,
    sleep,
    updateDeployment,
    USDC,
    walletPath,
} from './lib';

const ENTRY = 5n * USDC; // well below the 250 USDC hold threshold: paid out immediately
const PLAYER_SOL = 20_000_000n; // 0.02 SOL each: profile, match and vault rent
const SERVER_KEY = process.env.DEVNET_SERVER_KEY ?? '~/.config/solana/arenachain-devnet-server.json';

const wallet = await loadKeypair(walletPath());
const server = await loadKeypair(SERVER_KEY);
const client = await devnetClient(wallet);
const mint = address(readDeployment().usdcMint);
const [feeVault] = await findFeeVaultPda();
const send = async (label: string, plan: Parameters<typeof client.sendTransaction>[0]) => {
    await sleep(1200); // stay under the public devnet RPC rate limit
    // Retry only rate-limited failures that happened before submission, so nothing is sent twice.
    const sig = (await retry(() => client.sendTransaction(plan), failedBeforeSubmit)).context.signature;
    console.log(`${label.padEnd(26)} ${explorerTx(sig)}`);
    return sig;
};
const balance = async (token: Address) => (await retry(() => fetchToken(client.rpc, token))).data.amount;
const solBalance = async (a: Address) => (await retry(() => client.rpc.getBalance(a).send())).value;

/** Returns any SOL left on throwaway player keys to the wallet. */
async function sweep(signers: KeyPairSigner[]) {
    for (const s of signers) {
        const value = await solBalance(s.address);
        if (value > 0n) {
            await send(`return SOL ${s.address.slice(0, 6)}`, getTransferSolInstruction({ source: s, destination: wallet.address, amount: value }));
        }
    }
}

// 0. Recover SOL from earlier runs' players.
await sweep(await savedE2eKeypairs());
const usdc = (n: bigint) => `${Number(n) / Number(USDC)} USDC`;

// 1. Game server, registered once by the admin.
const [gameServer] = await findGameServerPda({ signerKey: server.address });
if (!(await retry(() => fetchMaybeGameServer(client.rpc, gameServer))).exists) {
    const operator = new Uint8Array(32);
    operator.set(new TextEncoder().encode('devnet-1'));
    await send('register game server', await getAdminRegisterServerInstructionAsync({
        admin: wallet, signerKey: server.address, operator,
    }));
}

// 2. Two players: SOL for rent, a USDC account with 10 test USDC, a profile.
type Player = { signer: KeyPairSigner; token: Address };
async function newPlayer(name: string): Promise<Player> {
    const signer = await newSavedKeypair(name);
    const [token] = await findAssociatedTokenPda({ owner: signer.address, mint, tokenProgram: TOKEN_PROGRAM_ADDRESS });
    await send(`fund ${name}`, [
        getTransferSolInstruction({ source: wallet, destination: signer.address, amount: lamports(PLAYER_SOL) }),
        await getCreateAssociatedTokenIdempotentInstructionAsync({ payer: wallet, owner: signer.address, mint }),
        getMintToCheckedInstruction({ mint, token, mintAuthority: wallet, amount: 10n * USDC, decimals: 6 }),
    ]);
    await send(`profile ${name}`, await getPlayerCreateProfileInstructionAsync({ authority: signer }));
    return { signer, token };
}
const a = await newPlayer('A');
const b = await newPlayer('B');

// 3. Create, join, start.
const nonce = BigInt(Date.now());
const [arenaMatch] = await findArenaMatchPda({ creator: a.signer.address, nonce });
await send('create match (A)', await getPlayerCreateMatchInstructionAsync({
    creator: a.signer, gameServer, mint, creatorToken: a.token, nonce, entryFee: ENTRY,
}));
await send('join match (B)', await getPlayerJoinMatchInstructionAsync({
    joiner: b.signer, arenaMatch, gameServer, mint, joinerToken: b.token,
}));
await send('start match (server)', await getServerStartMatchInstructionAsync({ serverSigner: server, arenaMatch }));

// 4. The server signs "B won"; the wallet relays it with the Ed25519 check right before submit.
const m = await retry(() => fetchMatch(client.rpc, arenaMatch));
const demoHash = new Uint8Array(32).fill(0xd0);
const message = getResultMessage({
    programAddress: ARENACHAIN_PROGRAM_ADDRESS, arenaMatch, createdSlot: m.data.createdSlot,
    kind: ResultKind.Win, winner: b.signer.address, demoHash,
});
const signature = await signBytes(server.keyPair.privateKey, message);
const feeBefore = await balance(feeVault);
const payoutSig = await send('result + payout', nonDivisibleSequentialInstructionPlan([
    getEd25519VerifyInstruction(server.address, signature, message),
    await getSubmitResultInstructionAsync({
        arenaMatch, gameServer, mint,
        playerAToken: a.token, playerBToken: b.token,
        playerAProfile: (await findProfilePda({ authority: a.signer.address }))[0],
        playerBProfile: (await findProfilePda({ authority: b.signer.address }))[0],
        feeVault, playerA: a.signer.address,
        kind: ResultKind.Win, winner: b.signer.address, demoHash,
    }),
]));

// 5. Check the money moved exactly as the rules say.
const pool = ENTRY * 2n;
const fee = (pool * 500n) / 10_000n;
const [aBal, bBal, feeAfter] = [await balance(a.token), await balance(b.token), await balance(feeVault)];
const settled = await retry(() => fetchMatch(client.rpc, arenaMatch));
const [vault] = await findVaultPda({ arenaMatch });
const vaultGone = (await retry(() => client.rpc.getAccountInfo(vault).send())).value === null;
const checks = {
    'match settled & paid': settled.data.status === MatchStatus.Settled && settled.data.paidOut,
    'winner B got pool - 5% fee': bBal === 10n * USDC - ENTRY + pool - fee,
    'loser A paid entry only': aBal === 10n * USDC - ENTRY,
    'fee vault +5%': feeAfter - feeBefore === fee,
    'vault closed': vaultGone,
};
console.log(`\nA: ${usdc(aBal)}  B: ${usdc(bBal)}  fee collected: ${usdc(feeAfter - feeBefore)}`);
for (const [k, ok] of Object.entries(checks)) console.log(`${ok ? 'PASS' : 'FAIL'}  ${k}`);

// 6. Return leftover player SOL to the wallet (the wallet pays the fee).
await sweep([a.signer, b.signer]);

updateDeployment({
    gameServer: { signerKey: server.address, account: gameServer },
    e2e: {
        match: arenaMatch, playerA: a.signer.address, playerB: b.signer.address,
        payoutSignature: payoutSig, checks: Object.fromEntries(Object.entries(checks).map(([k, v]) => [k, v])),
    },
});
console.log(`\nMatch account: ${explorerAddress(arenaMatch)}`);
if (!Object.values(checks).every(Boolean)) process.exit(1);
