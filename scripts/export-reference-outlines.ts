/**
 * Exporte des contours de référence depuis l'implémentation TypeScript.
 *
 * Ces contours servent d'oracle au test Rust `tests/parity.rs`. Sans eux, les
 * deux implémentations peuvent diverger sans que rien ne le signale : l'aperçu
 * à l'écran montrerait alors une forme différente de celle écrite dans le DDS.
 *
 *   node scripts/export-reference-outlines.ts
 */
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { defaultShape, outline, type MinimapShape } from '../src/lib/shape.ts'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const out = join(root, 'crates/core/tests/fixtures')

const cases: { name: string; shape: MinimapShape }[] = [
  {
    name: 'vanilla',
    shape: {
      ...defaultShape(),
      preset: 'vanilla',
      corners: { tl: 0, tr: 0, br: 0, bl: 0 },
      smoothing: 0,
    },
  },
  { name: 'rounded', shape: { ...defaultShape(), preset: 'rounded' } },
  {
    name: 'circle',
    shape: {
      ...defaultShape(),
      preset: 'circle',
      corners: { tl: 1, tr: 1, br: 1, bl: 1 },
      smoothing: 0,
    },
  },
  {
    name: 'squircle',
    shape: {
      ...defaultShape(),
      preset: 'squircle',
      corners: { tl: 1, tr: 1, br: 1, bl: 1 },
      smoothing: 1,
    },
  },
  {
    // Rayons asymétriques : couvre la mise à l'échelle façon CSS.
    name: 'asymetrique',
    shape: {
      ...defaultShape(),
      preset: 'rounded',
      corners: { tl: 0.9, tr: 0.1, br: 0.6, bl: 0.35 },
      smoothing: 0.42,
      inset: { top: 0.05, right: 0.12, bottom: 0.03, left: 0.08 },
    },
  },
  {
    name: 'hexagone',
    shape: {
      ...defaultShape(),
      preset: 'polygon',
      corners: { tl: 0.12, tr: 0.12, br: 0.12, bl: 0.12 },
      polygon: { sides: 6, rotation: 0.3 },
    },
  },
]

const SAMPLES = 32 // identique à mask.rs, pour comparer ce qui est réellement rasterisé

const payload = cases.map(({ name, shape }) => ({
  name,
  shape,
  samples: SAMPLES,
  points: outline(shape, SAMPLES).map((p) => [p.x, p.y]),
}))

mkdirSync(out, { recursive: true })
writeFileSync(join(out, 'outlines.json'), JSON.stringify(payload, null, 2) + '\n')
console.log(`${payload.length} contours écrits dans ${out}/outlines.json`)
