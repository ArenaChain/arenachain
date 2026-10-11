import path from 'node:path';
import type { NextConfig } from 'next';

const config: NextConfig = {
    // The generated program client ships as TypeScript source from the workspace.
    transpilePackages: ['@arenachain/client'],
    // Repo root, so the app can read ../deployments/devnet.json and the workspace client.
    turbopack: { root: path.join(import.meta.dirname, '..') },
    outputFileTracingRoot: path.join(import.meta.dirname, '..'),
};

export default config;
