'use client';

// Mounted once in the root layout, fixed behind all content. Picks the cheapest thing that
// still looks right: animated 3D stage, a single still frame (reduced motion), or a static
// gradient when WebGL isn't available.

import dynamic from 'next/dynamic';
import { useEffect, useState } from 'react';

const StageScene = dynamic(() => import('./StageScene'), { ssr: false });

type Mode = 'pending' | 'static' | 'still' | 'animated';

function hasWebGL(): boolean {
    try {
        const canvas = document.createElement('canvas');
        return !!(canvas.getContext('webgl2') ?? canvas.getContext('webgl'));
    } catch {
        return false;
    }
}

export function Background() {
    const [mode, setMode] = useState<Mode>('pending');
    const [visible, setVisible] = useState(true);

    useEffect(() => {
        const reduce = window.matchMedia('(prefers-reduced-motion: reduce)');
        const decide = () => setMode(!hasWebGL() ? 'static' : reduce.matches ? 'still' : 'animated');
        decide();
        reduce.addEventListener('change', decide);
        // Pause rendering entirely while the tab is hidden.
        const onVisibility = () => setVisible(document.visibilityState === 'visible');
        document.addEventListener('visibilitychange', onVisibility);
        return () => {
            reduce.removeEventListener('change', decide);
            document.removeEventListener('visibilitychange', onVisibility);
        };
    }, []);

    return (
        <div className="stage-bg" aria-hidden="true">
            {(mode === 'animated' || mode === 'still') && <StageScene animate={mode === 'animated' && visible} />}
            <div className="stage-vignette" />
        </div>
    );
}
