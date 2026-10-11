'use client';

import { createContext, type ReactNode, useCallback, useContext, useState } from 'react';

import { explorerTx } from '@/lib/format';

type Toast = { id: number; tone: 'ok' | 'error' | 'info'; title: string; signature?: string };
const Ctx = createContext<(t: Omit<Toast, 'id'>) => void>(() => {});

export const useToast = () => useContext(Ctx);

export function ToastProvider({ children }: { children: ReactNode }) {
    const [toasts, setToasts] = useState<Toast[]>([]);
    const push = useCallback((t: Omit<Toast, 'id'>) => {
        const id = Date.now() + Math.random();
        setToasts((all) => [...all.slice(-3), { ...t, id }]);
        setTimeout(() => setToasts((all) => all.filter((x) => x.id !== id)), t.tone === 'error' ? 12000 : 9000);
    }, []);
    return (
        <Ctx.Provider value={push}>
            {children}
            <div className="toasts" role="status" aria-live="polite">
                {toasts.map((t) => (
                    <div key={t.id} className={`toast toast-${t.tone}`}>
                        <span>{t.title}</span>
                        {t.signature && (
                            <a href={explorerTx(t.signature)} target="_blank" rel="noreferrer">
                                View on Explorer
                            </a>
                        )}
                    </div>
                ))}
            </div>
        </Ctx.Provider>
    );
}
