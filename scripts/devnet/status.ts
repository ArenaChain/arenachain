// Read-only view of the devnet deployment. Usage: npm run devnet:status

import { fetchToken } from '@solana-program/token';

import { fetchGlobalConfig, findConfigPda, findFeeVaultPda } from '../../clients/js/src';
import { devnetClient, loadKeypair, USDC, walletPath } from './lib';

const wallet = await loadKeypair(walletPath());
const client = await devnetClient(wallet);
const [config] = await findConfigPda();
const [feeVault] = await findFeeVaultPda();
const c = (await fetchGlobalConfig(client.rpc, config)).data;
const vault = (await fetchToken(client.rpc, feeVault)).data;
const { value: lamports } = await client.rpc.getBalance(wallet.address).send();

console.log(`Config ${config}
  admin             ${c.admin}
  pending admin     ${c.pendingAdmin}
  dispute authority ${c.disputeAuthority}
  treasury          ${c.treasury}
  usdc mint         ${c.usdcMint}
  hold threshold    ${c.holdThreshold / USDC} USDC
  hold              ${c.holdSecs}s
  result window     ${c.resultWindowSecs}s
  paused            ${c.paused}
Fee vault ${feeVault}: ${Number(vault.amount) / Number(USDC)} USDC
Wallet ${wallet.address}: ${Number(lamports) / 1e9} SOL`);
