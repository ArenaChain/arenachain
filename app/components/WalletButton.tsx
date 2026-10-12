'use client';

import {
    useConnect,
    useConnectedWallet,
    useDisconnect,
    useWallets,
    WalletReadyGate,
} from '@solana/kit-plugin-wallet/react';
import { useClient } from '@solana/react';
import { useEffect, useRef, useState } from 'react';

import type { AppClient } from '@/app/providers';
import { explorerAddress, formatUsdc, short } from '@/lib/format';
import { useWalletView } from '@/lib/hooks';
import { useToast } from './Toasts';

/** True after the first client render. Wallet state only exists in the browser, so anything that
 *  depends on it renders after mount to keep server and client HTML identical. */
function useMounted() {
    const [mounted, setMounted] = useState(false);
    useEffect(() => setMounted(true), []);
    return mounted;
}

/** Closes a popover on outside click or Escape. */
function useDismiss(open: boolean, close: () => void) {
    const ref = useRef<HTMLDivElement>(null);
    useEffect(() => {
        if (!open) return;
        const onDown = (e: MouseEvent) => {
            if (ref.current && !ref.current.contains(e.target as Node)) close();
        };
        const onKey = (e: KeyboardEvent) => e.key === 'Escape' && close();
        document.addEventListener('mousedown', onDown);
        document.addEventListener('keydown', onKey);
        return () => {
            document.removeEventListener('mousedown', onDown);
            document.removeEventListener('keydown', onKey);
        };
    }, [open, close]);
    return ref;
}

function WalletPicker({ label, className }: { label: string; className: string }) {
    const client = useClient<AppClient>();
    const wallets = useWallets(client);
    const { dispatch: connect, isRunning: connecting } = useConnect(client);
    const [open, setOpen] = useState(false);
    const ref = useDismiss(open, () => setOpen(false));
    return (
        <div className="wallet" ref={ref}>
            <button className={className} onClick={() => setOpen((o) => !o)} aria-expanded={open} aria-haspopup="menu">
                {connecting ? 'Connecting…' : label}
            </button>
            {open && (
                <div className="menu" role="menu">
                    {wallets.length === 0 && (
                        <p className="menu-note">
                            No Solana wallet found. Install Phantom, Solflare or Backpack and switch it to devnet.
                        </p>
                    )}
                    {wallets.map((w) => (
                        <button
                            key={w.name}
                            role="menuitem"
                            className="menu-item"
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

function ConnectedMenu() {
    const client = useClient<AppClient>();
    const connected = useConnectedWallet(client);
    const { dispatch: disconnect } = useDisconnect(client);
    const toast = useToast();
    const address = connected?.account.address ?? null;
    const { data: w } = useWalletView(address);
    const [open, setOpen] = useState(false);
    const ref = useDismiss(open, () => setOpen(false));
    if (!connected || !address) return null;

    async function copy() {
        setOpen(false);
        try {
            await navigator.clipboard.writeText(address!);
            toast({ tone: 'ok', title: 'Address copied' });
        } catch {
            toast({ tone: 'error', title: "Couldn't copy. Select the address on Explorer instead." });
        }
    }

    return (
        <div className="wallet" ref={ref}>
            <button
                className="wallet-pill"
                onClick={() => setOpen((o) => !o)}
                aria-expanded={open}
                aria-haspopup="menu"
                title={address}
            >
                {connected.wallet.icon && <img src={connected.wallet.icon} alt="" width={18} height={18} />}
                <span className="wallet-addr">{short(address)}</span>
                <span className="wallet-bal">{w ? `${formatUsdc(w.usdc)} USDC` : '…'}</span>
                <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
                    <path d="M3 4.5 6 7.5 9 4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
                </svg>
            </button>
            {open && (
                <div className="menu" role="menu">
                    <button role="menuitem" className="menu-item" onClick={copy}>
                        Copy address
                    </button>
                    <a
                        role="menuitem"
                        className="menu-item"
                        href={explorerAddress(address)}
                        target="_blank"
                        rel="noreferrer"
                        onClick={() => setOpen(false)}
                    >
                        View on Explorer
                    </a>
                    <button
                        role="menuitem"
                        className="menu-item menu-danger"
                        onClick={() => {
                            setOpen(false);
                            disconnect();
                        }}
                    >
                        Disconnect
                    </button>
                </div>
            )}
        </div>
    );
}

function HeaderWallet() {
    const client = useClient<AppClient>();
    const connected = useConnectedWallet(client);
    return connected ? <ConnectedMenu /> : <WalletPicker label="Connect wallet" className="btn btn-primary" />;
}

/** Header wallet control: picker when disconnected, address + balance menu when connected. */
export function WalletButton() {
    const client = useClient<AppClient>();
    const mounted = useMounted();
    if (!mounted) return <span className="wallet-skeleton skeleton" aria-hidden="true" />;
    return (
        <WalletReadyGate client={client} fallback={<span className="wallet-skeleton skeleton" aria-hidden="true" />}>
            <HeaderWallet />
        </WalletReadyGate>
    );
}

/** A large "Connect wallet" call to action that opens the same picker (hero). */
export function ConnectButton() {
    const client = useClient<AppClient>();
    const mounted = useMounted();
    if (!mounted) return <span className="btn btn-primary btn-lg is-placeholder">Connect wallet</span>;
    return (
        <WalletReadyGate
            client={client}
            fallback={<span className="btn btn-primary btn-lg is-placeholder">Connect wallet</span>}
        >
            <WalletPicker label="Connect wallet" className="btn btn-primary btn-lg" />
        </WalletReadyGate>
    );
}
