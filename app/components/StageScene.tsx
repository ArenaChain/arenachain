'use client';

// "Esports stage at night": a low-poly wireframe arena floor in perspective with soft glowing
// particles drifting upward, tinted with the brand gradient. Loaded client-only by Background.tsx.

import { Canvas, useFrame, useThree } from '@react-three/fiber';
import { useEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';

const BG = new THREE.Color('#040813'); // Void
// Brand gradient: Azure -> Indigo -> Violet.
const C1 = new THREE.Color('#0485E0');
const C2 = new THREE.Color('#4448E7');
const C3 = new THREE.Color('#5434ED');

function brand(t: number, out: THREE.Color) {
    const k = Math.min(1, Math.max(0, t));
    return k < 0.5 ? out.copy(C1).lerp(C2, k * 2) : out.copy(C2).lerp(C3, (k - 0.5) * 2);
}

/** Wireframe floor that rises gently toward the edges, like an arena bowl. */
function ArenaFloor() {
    const geometry = useMemo(() => {
        const plane = new THREE.PlaneGeometry(90, 70, 36, 28);
        plane.rotateX(-Math.PI / 2);
        const pos = plane.attributes.position;
        for (let i = 0; i < pos.count; i++) {
            const x = pos.getX(i);
            const z = pos.getZ(i);
            const r = Math.hypot(x / 45, z / 35);
            // Flat center (the stage), then a bowl, with a little low-poly jitter.
            const rise = Math.max(0, r - 0.32) ** 2 * 16;
            const jitter = (Math.sin(x * 1.7 + z * 0.9) + Math.cos(z * 1.3 - x * 0.6)) * 0.12 * Math.min(1, r * 2);
            pos.setY(i, rise + jitter);
        }
        const wire = new THREE.WireframeGeometry(plane);
        const wp = wire.attributes.position;
        const colors = new Float32Array(wp.count * 3);
        const c = new THREE.Color();
        for (let i = 0; i < wp.count; i++) {
            brand((wp.getX(i) + 45) / 90 + wp.getZ(i) / 140, c);
            colors.set([c.r, c.g, c.b], i * 3);
        }
        wire.setAttribute('color', new THREE.BufferAttribute(colors, 3));
        plane.dispose();
        return wire;
    }, []);
    useEffect(() => () => geometry.dispose(), [geometry]);
    return (
        <lineSegments geometry={geometry} position={[0, -4, -14]}>
            <lineBasicMaterial vertexColors transparent opacity={0.32} fog />
        </lineSegments>
    );
}

/** A faint ring marking the stage. */
function StageRing() {
    const geometry = useMemo(() => {
        const pts: THREE.Vector3[] = [];
        for (let i = 0; i <= 96; i++) {
            const a = (i / 96) * Math.PI * 2;
            pts.push(new THREE.Vector3(Math.cos(a) * 12, 0, Math.sin(a) * 9));
        }
        return new THREE.BufferGeometry().setFromPoints(pts);
    }, []);
    useEffect(() => () => geometry.dispose(), [geometry]);
    return (
        <lineLoop geometry={geometry} position={[0, -3.9, -14]}>
            <lineBasicMaterial color="#1062B6" transparent opacity={0.5} fog />
        </lineLoop>
    );
}

const PARTICLES = 1400; // brief: under 2k
const HEIGHT = 26;

/** Particles drift upward entirely on the GPU: no per-frame CPU work besides one uniform. */
function Particles() {
    const material = useRef<THREE.ShaderMaterial>(null);
    const geometry = useMemo(() => {
        const g = new THREE.BufferGeometry();
        const base = new Float32Array(PARTICLES * 3);
        const color = new Float32Array(PARTICLES * 3);
        const seed = new Float32Array(PARTICLES);
        const c = new THREE.Color();
        for (let i = 0; i < PARTICLES; i++) {
            const x = (Math.random() - 0.5) * 70;
            base.set([x, Math.random() * HEIGHT, -Math.random() * 40 + 4], i * 3);
            brand((x + 35) / 70 + (Math.random() - 0.5) * 0.3, c);
            color.set([c.r, c.g, c.b], i * 3);
            seed[i] = Math.random();
        }
        g.setAttribute('position', new THREE.BufferAttribute(base, 3));
        g.setAttribute('color', new THREE.BufferAttribute(color, 3));
        g.setAttribute('seed', new THREE.BufferAttribute(seed, 1));
        return g;
    }, []);
    const uniforms = useMemo(() => ({ uTime: { value: 0 }, uHeight: { value: HEIGHT } }), []);
    useEffect(() => () => geometry.dispose(), [geometry]);
    useFrame((_, delta) => {
        if (material.current) material.current.uniforms.uTime.value += Math.min(delta, 0.05);
    });
    return (
        <points geometry={geometry} position={[0, -4, -10]}>
            <shaderMaterial
                ref={material}
                uniforms={uniforms}
                transparent
                depthWrite={false}
                blending={THREE.AdditiveBlending}
                vertexShader={`
                    attribute vec3 color;
                    attribute float seed;
                    uniform float uTime;
                    uniform float uHeight;
                    varying vec3 vColor;
                    varying float vFade;
                    void main() {
                        vec3 p = position;
                        float speed = 0.35 + seed * 0.55;
                        p.y = mod(p.y + uTime * speed, uHeight);
                        p.x += sin(uTime * 0.25 + seed * 6.28) * 0.6;
                        vec4 mv = modelViewMatrix * vec4(p, 1.0);
                        gl_Position = projectionMatrix * mv;
                        gl_PointSize = (1.5 + seed * 2.5) * (60.0 / -mv.z);
                        // Fade in at the floor and out near the top.
                        vFade = smoothstep(0.0, 2.5, p.y) * (1.0 - smoothstep(uHeight * 0.6, uHeight, p.y));
                        vColor = color;
                    }
                `}
                fragmentShader={`
                    varying vec3 vColor;
                    varying float vFade;
                    void main() {
                        float d = length(gl_PointCoord - 0.5);
                        float a = smoothstep(0.5, 0.0, d);
                        gl_FragColor = vec4(vColor, a * a * 0.75 * vFade);
                    }
                `}
            />
        </points>
    );
}

/** Very slow camera drift, so the stage feels alive without drawing attention. */
function CameraDrift() {
    const { camera } = useThree();
    const t = useRef(0);
    useFrame((_, delta) => {
        t.current += Math.min(delta, 0.05);
        camera.position.x = Math.sin(t.current * 0.05) * 2.2;
        camera.position.y = 3.2 + Math.sin(t.current * 0.07) * 0.4;
        camera.lookAt(0, -1.5, -14);
    });
    return null;
}

export default function StageScene({ animate }: { animate: boolean }) {
    return (
        <Canvas
            dpr={[1, 1.5]}
            frameloop={animate ? 'always' : 'demand'}
            camera={{ position: [0, 3.2, 10], fov: 55, near: 0.1, far: 120 }}
            gl={{ antialias: false, alpha: false, powerPreference: 'low-power' }}
            onCreated={({ scene, gl }) => {
                scene.background = BG;
                scene.fog = new THREE.Fog(BG, 18, 62);
                gl.setClearColor(BG);
            }}
        >
            <ArenaFloor />
            <StageRing />
            <Particles />
            {animate && <CameraDrift />}
        </Canvas>
    );
}
