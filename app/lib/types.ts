// JSON shapes the app's API routes return (bigints as strings).

export type MatchStatusName = 'Open' | 'Full' | 'InProgress' | 'Settled' | 'Refunded' | 'Disputed';
export type ResultKindName = 'Win' | 'Forfeit' | 'Void';

export type MatchView = {
    address: string;
    playerA: string;
    playerB: string | null;
    gameServer: string;
    entryFee: string;
    status: MatchStatusName;
    createdAt: number;
    joinedAt: number;
    startedAt: number;
    startDeadline: number;
    resultDeadline: number;
    resultKind: ResultKindName | null;
    winner: string | null;
    unlockAt: number;
    paidOut: boolean;
};

export type ProfileView = {
    authority: string;
    elo: number;
    wins: number;
    losses: number;
    totalForfeits: number;
    reputation: number;
    bannedUntil: number;
};

export type WalletView = {
    wallet: string;
    sol: string;
    usdc: string;
    hasUsdcAccount: boolean;
    profile: ProfileView | null;
};

export type TxView = {
    signature: string;
    label: string;
    blockTime: number | null;
    ok: boolean;
};

export type Settlement = {
    pool: string;
    fee: string;
    payout: string;
    /** For Void: what each player got back. */
    refundEach: string | null;
    held: boolean;
};

export type MatchDetail = {
    match: MatchView;
    profiles: Record<string, ProfileView | null>;
    settlement: Settlement | null;
    txs: TxView[];
    serverKey: string;
    now: number;
};
