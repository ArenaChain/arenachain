'use client';

import {
    useConnect,
    useConnectedWallet,
    useDisconnect,
    useWallets,
    WalletReadyGate,
} from '@solana/kit-plugin-wallet/react';
import { useClient } from '@solana/react';
import { useState } from 'react';

import type { AppClient } from '@/app/providers';
import { short } from '@/lib/format';

function Picker() {
    const client = useClient<AppClient>();
    const wallets = useWallets(client);
    const connected = useConnectedWallet(client);
    const { dispatch: connect, isRunning: connecting } = useConnect(client);
    const { dispatch: disconnect } = useDisconnect(client);
    const [open, setOpen] = useState(false);

    if (connected) {
        return (
            <div className="wallet">
                <span className="wallet-addr" title={connected.account.address}>
                    {connected.wallet.icon && <img src={connected.wallet.icon} alt="" width={18} height={18} />}
                    {short(connected.account.address)}
                </span>
                <button className="btn btn-ghost btn-sm" onClick={() => disconnect()}>
                    Disconnect
                </button>
            </div>
        );
    }
    return (
        <div className="wallet">
            <button className="btn btn-primary" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
                {connecting ? 'Connecting…' : 'Connect wallet'}
            </button>
            {open && (
                <div className="wallet-menu" role="menu">
                    {wallets.length === 0 && (
                        <p className="muted">
                            No Solana wallet found. Install Phantom, Solflare or Backpack and switch it to devnet.
                        </p>
                    )}
                    {wallets.map((w) => (
                        <button
                            key={w.name}
                            role="menuitem"
                            className="wallet-option"
                            onClick={() => {
                                setOpen(false);
                                connect(w);
                            }}
                        >
                            {w.icon && <img src={w.icon} alt="" width={22} height={22} />}
                            {w.name}
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}

export function WalletButton() {
    const client = useClient<AppClient>();
    return (
        <WalletReadyGate client={client} fallback={<span className="muted">Finding wallets…</span>}>
            <Picker />
        </WalletReadyGate>
    );
}
