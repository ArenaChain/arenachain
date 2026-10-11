'use client';

import { createContext, type ReactNode, useCallback, useContext, useState } from 'react';

import { explorerTx } from '@/lib/format';

type Toast = { id: number; tone: 'ok' | 'error' | 'info'; title: string; signature?: string };
const Ctx = createContext<(t: Omit<Toast, 'id'>) => void>(() => {});

export const useToast = () => useContext(Ctx);

export function ToastProvider({ children }: { children: ReactNode }) {
    const [toasts, setToasts] = useState<Toast[]>([]);
    const dismiss = useCallback((id: number) => setToasts((all) => all.filter((x) => x.id !== id)), []);
    const push = useCallback(
        (t: Omit<Toast, 'id'>) => {
            const id = Date.now() + Math.random();
            setToasts((all) => [...all.slice(-3), { ...t, id }]);
            // A toast with a transaction keeps its Explorer link on screen until it's closed.
            if (!t.signature) setTimeout(() => dismiss(id), t.tone === 'error' ? 12000 : 8000);
        },
        [dismiss],
    );
    return (
        <Ctx.Provider value={push}>
            {children}
            <div className="toasts" role="status" aria-live="polite">
                {toasts.map((t) => (
                    <div key={t.id} className={`toast toast-${t.tone}`}>
                        <span>{t.title}</span>
                        {t.signature && (
                            <a href={explorerTx(t.signature)} target="_blank" rel="noreferrer">
                                View transaction on Solana Explorer (devnet)
                            </a>
                        )}
                        <button className="toast-close" onClick={() => dismiss(t.id)} aria-label="Close">
                            ×
                        </button>
                    </div>
                ))}
            </div>
        </Ctx.Provider>
    );
}
