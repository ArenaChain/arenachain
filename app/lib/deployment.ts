// Public devnet deployment facts (addresses only), shared by browser and server code.
import { address } from '@solana/kit';

import devnet from '../../deployments/devnet.json';

export const PROGRAM_ID = address(devnet.programId);
export const USDC_MINT = address(devnet.usdcMint);
export const TREASURY = address(devnet.treasury);
export const FEE_VAULT = address(devnet.feeVault);
/** The game server this demo assigns every match to (registered on-chain). */
export const GAME_SERVER = address(devnet.gameServer.account);
export const GAME_SERVER_KEY = address(devnet.gameServer.signerKey);
export const HOLD_THRESHOLD = BigInt(devnet.configParams.holdThreshold);

export const CHAIN = 'solana:devnet' as const;
export const PUBLIC_RPC_URL = process.env.NEXT_PUBLIC_SOLANA_RPC_URL ?? 'https://api.devnet.solana.com';
