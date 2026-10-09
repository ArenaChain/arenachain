// End-to-end flow on an embedded Surfpool surfnet, driven through the Codama Kit client.
// Run with `npm run test:e2e` after `anchor build` (also part of `anchor test`).

import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { after, before, test } from 'node:test';

import { fetchToken, getMintEncoder, TOKEN_PROGRAM_ADDRESS } from '@solana-program/token';
import {
    type Address,
    createClient,
    generateKeyPairSigner,
    getAddressEncoder,
    getProgramDerivedAddress,
    type KeyPairSigner,
    address,
    nonDivisibleSequentialInstructionPlan,
    none,
    signBytes,
    some,
} from '@solana/kit';
import { surfpool } from '@solana/surfpool/kit';

import {
    ARENACHAIN_PROGRAM_ADDRESS,
    MatchStatus,
    ResultKind,
    fetchMatch,
    findArenaMatchPda,
    findFeeVaultPda,
    findGameServerPda,
    findProfilePda,
    findVaultPda,
    fetchPlayerProfile,
    getAdminWithdrawFeesInstructionAsync,
    getRefundExpiredInstructionAsync,
    getAdminInitializeConfigInstructionAsync,
    getAdminRegisterServerInstructionAsync,
    getClaimPayoutInstructionAsync,
    getEd25519VerifyInstruction,
    getPlayerCreateMatchInstructionAsync,
    getPlayerCreateProfileInstructionAsync,
    getPlayerJoinMatchInstructionAsync,
    getResultMessage,
    getServerStartMatchInstructionAsync,
    getSubmitResultInstructionAsync,
} from '../clients/js/src';

const LOADER_V3 = address('BPFLoaderUpgradeab1e11111111111111111111111');
const USDC = 1_000_000n;
const HOLD_THRESHOLD = 100n * USDC;
const HOLD_SECS = 3600n;

const client = await createClient().use(surfpool({ surfnet: { offline: true } }));
after(() => client.surfnet.stop());

let mint: Address;
let server: KeyPairSigner;
let gameServer: Address;
let feeVault: Address;
let treasury: KeyPairSigner;

type Player = { signer: KeyPairSigner; token: Address };

async function player(usdc: bigint): Promise<Player> {
    const signer = await generateKeyPairSigner();
    client.surfnet.fundSol(signer.address, 1_000_000_000);
    client.surfnet.fundToken(signer.address, mint, Number(usdc));
    const token = address(client.surfnet.getAta(signer.address, mint));
    await client.sendTransaction(await getPlayerCreateProfileInstructionAsync({ authority: signer }));
    return { signer, token };
}

async function balance(token: Address): Promise<bigint> {
    return (await fetchToken(client.rpc, token)).data.amount;
}

async function startedMatch(a: Player, b: Player, nonce: bigint, entryFee: bigint): Promise<Address> {
    const [arenaMatch] = await findArenaMatchPda({ creator: a.signer.address, nonce });
    await client.sendTransaction(
        await getPlayerCreateMatchInstructionAsync({
            creator: a.signer,
            gameServer,
            mint,
            creatorToken: a.token,
            nonce,
            entryFee,
        }),
    );
    await client.sendTransaction(
        await getPlayerJoinMatchInstructionAsync({ joiner: b.signer, arenaMatch, mint, joinerToken: b.token }),
    );
    await client.sendTransaction(await getServerStartMatchInstructionAsync({ serverSigner: server, arenaMatch }));
    return arenaMatch;
}

async function submitSignedResult(arenaMatch: Address, a: Player, b: Player, kind: ResultKind, winner: Address) {
    const m = await fetchMatch(client.rpc, arenaMatch);
    const demoHash = new Uint8Array(32).fill(7);
    const message = getResultMessage({
        programAddress: ARENACHAIN_PROGRAM_ADDRESS,
        arenaMatch,
        createdSlot: m.data.createdSlot,
        kind,
        winner,
        demoHash,
    });
    // Done by the result service in production.
    const signature = await signBytes(server.keyPair.privateKey, message);
    const submit = await getSubmitResultInstructionAsync({
        arenaMatch,
        gameServer,
        mint,
        playerAToken: a.token,
        playerBToken: b.token,
        feeVault,
        playerA: a.signer.address,
        playerAProfile: (await findProfilePda({ authority: a.signer.address }))[0],
        playerBProfile: (await findProfilePda({ authority: b.signer.address }))[0],
        kind,
        winner,
        demoHash,
    });
    // One transaction, signature check immediately before submit_result.
    await client.sendTransaction(
        nonDivisibleSequentialInstructionPlan([
            getEd25519VerifyInstruction(server.address, signature, message),
            submit,
        ]),
    );
}

before(async () => {
    const soPath = resolve(import.meta.dirname, '../target/deploy/arenachain.so');
    client.surfnet.deploy({ programId: ARENACHAIN_PROGRAM_ADDRESS, soPath });

    // Make the test payer the upgrade authority (only it may initialize the config).
    const [programData] = await getProgramDerivedAddress({
        programAddress: LOADER_V3,
        seeds: [getAddressEncoder().encode(ARENACHAIN_PROGRAM_ADDRESS)],
    });
    const pd = await client.rpc.getAccountInfo(programData, { encoding: 'base64' }).send();
    assert.ok(pd.value, 'program data account exists after deploy');
    const data = Buffer.from(pd.value.data[0], 'base64');
    data.writeUInt32LE(3, 0);
    data[12] = 1;
    data.set(getAddressEncoder().encode(client.payer.address), 13);
    client.surfnet.setAccount(programData, Number(pd.value.lamports), data, LOADER_V3);

    // Test USDC mint (classic SPL Token).
    mint = (await generateKeyPairSigner()).address;
    const mintData = getMintEncoder().encode({
        mintAuthority: some(client.payer.address),
        supply: 0n,
        decimals: 6,
        isInitialized: true,
        freezeAuthority: none(),
    });
    const rent = await client.rpc.getMinimumBalanceForRentExemption(82n).send();
    client.surfnet.setAccount(mint, Number(rent), new Uint8Array(mintData), TOKEN_PROGRAM_ADDRESS);

    treasury = await generateKeyPairSigner();
    await client.sendTransaction(
        await getAdminInitializeConfigInstructionAsync({
            authority: client.payer,
            usdcMint: mint,
            programData,
            admin: client.payer.address,
            disputeAuthority: client.payer.address,
            treasury: treasury.address,
            params: { holdThreshold: HOLD_THRESHOLD, holdSecs: HOLD_SECS, resultWindowSecs: 3n * 3600n },
        }),
    );

    server = await generateKeyPairSigner();
    client.surfnet.fundSol(server.address, 1_000_000_000);
    await client.sendTransaction(
        await getAdminRegisterServerInstructionAsync({
            admin: client.payer,
            signerKey: server.address,
            operator: new Uint8Array(32),
        }),
    );
    [gameServer] = await findGameServerPda({ signerKey: server.address });
    [feeVault] = await findFeeVaultPda();
});

test('create → join → start → server-signed Win pays winner minus 5% fee', async () => {
    const entry = 10n * USDC;
    const a = await player(entry);
    const b = await player(entry);
    const arenaMatch = await startedMatch(a, b, 1n, entry);
    assert.equal((await fetchMatch(client.rpc, arenaMatch)).data.status, MatchStatus.InProgress);

    const feeBefore = await balance(feeVault);
    await submitSignedResult(arenaMatch, a, b, ResultKind.Win, b.signer.address);

    const pool = entry * 2n;
    const fee = (pool * 500n) / 10_000n;
    assert.equal(await balance(b.token), pool - fee);
    assert.equal(await balance(a.token), 0n);
    assert.equal((await balance(feeVault)) - feeBefore, fee);
    const m = await fetchMatch(client.rpc, arenaMatch);
    assert.equal(m.data.status, MatchStatus.Settled);
    assert.equal(m.data.paidOut, true);
    const [vault] = await findVaultPda({ arenaMatch });
    const vaultInfo = await client.rpc.getAccountInfo(vault).send();
    assert.equal(vaultInfo.value, null, 'vault closed');
});

test('held payout: claim fails before unlock, succeeds after time travel', async () => {
    const entry = HOLD_THRESHOLD / 2n;
    const a = await player(entry);
    const b = await player(entry);
    const arenaMatch = await startedMatch(a, b, 1n, entry);
    await submitSignedResult(arenaMatch, a, b, ResultKind.Win, a.signer.address);

    const held = await fetchMatch(client.rpc, arenaMatch);
    assert.equal(held.data.status, MatchStatus.Settled);
    assert.equal(held.data.paidOut, false);
    assert.equal(await balance(a.token), 0n);

    const claim = () =>
        getClaimPayoutInstructionAsync({
            arenaMatch,
            mint,
            winnerToken: a.token,
            feeVault,
            playerA: a.signer.address,
        });
    await assert.rejects(async () => client.sendTransaction(await claim()));

    client.surfnet.timeTravelToTimestamp(Number(held.data.unlockAt + 60n) * 1000);
    await client.sendTransaction(await claim());

    const pool = entry * 2n;
    assert.equal(await balance(a.token), pool - (pool * 500n) / 10_000n);
    assert.equal((await fetchMatch(client.rpc, arenaMatch)).data.paidOut, true);
});

test('Forfeit updates Elo, record and reputation', async () => {
    const entry = 5n * USDC;
    const a = await player(entry);
    const b = await player(entry);
    const arenaMatch = await startedMatch(a, b, 1n, entry);
    await submitSignedResult(arenaMatch, a, b, ResultKind.Forfeit, a.signer.address);
    const pa = await fetchPlayerProfile(client.rpc, (await findProfilePda({ authority: a.signer.address }))[0]);
    const pb = await fetchPlayerProfile(client.rpc, (await findProfilePda({ authority: b.signer.address }))[0]);
    assert.equal(pa.data.elo, 1016);
    assert.equal(pb.data.elo, 984);
    assert.equal(pa.data.wins, 1);
    assert.equal(pb.data.losses, 1);
    assert.equal(pb.data.reputation, 90);
    assert.equal(pb.data.totalForfeits, 1);
});

test('expired match: anyone refunds both players after the deadline', async () => {
    const entry = 7n * USDC;
    const a = await player(entry);
    const b = await player(entry);
    const arenaMatch = await startedMatch(a, b, 1n, entry);
    const m = await fetchMatch(client.rpc, arenaMatch);

    const refund = () =>
        getRefundExpiredInstructionAsync({
            arenaMatch,
            mint,
            playerAToken: a.token,
            playerBToken: b.token,
            playerA: a.signer.address,
        });
    await assert.rejects(async () => client.sendTransaction(await refund()));

    client.surfnet.timeTravelToTimestamp(Number(m.data.resultDeadline + 60n) * 1000);
    await client.sendTransaction(await refund());
    assert.equal(await balance(a.token), entry);
    assert.equal(await balance(b.token), entry);
    assert.equal((await fetchMatch(client.rpc, arenaMatch)).data.status, MatchStatus.Refunded);
});

test('admin withdraws collected fees to the treasury', async () => {
    const collected = await balance(feeVault);
    assert.ok(collected > 0n, 'earlier tests collected fees');
    client.surfnet.fundToken(treasury.address, mint, 0);
    const treasuryToken = address(client.surfnet.getAta(treasury.address, mint));
    await client.sendTransaction(
        await getAdminWithdrawFeesInstructionAsync({
            admin: client.payer,
            mint,
            treasuryToken,
            amount: collected,
        }),
    );
    assert.equal(await balance(treasuryToken), collected);
    assert.equal(await balance(feeVault), 0n);
});
