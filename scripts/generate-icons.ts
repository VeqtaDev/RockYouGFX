/**
 * Génère les icônes de l'application.
 *
 * L'icône est dessinée par `outline()`, le même code que celui qui produit les
 * masques : elle reste donc cohérente avec le projet, et toute régression du
 * moteur de forme se voit jusque dans l'icône.
 *
 *   node scripts/generate-icons.ts
 */
import { deflateSync } from 'node:zlib'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { defaultShape, outline, type Vec2 } from '../src/lib/shape.ts'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const outDir = join(root, 'src-tauri/icons')

// --- PNG ---------------------------------------------------------------------

const CRC_TABLE = (() => {
  const t = new Uint32Array(256)
  for (let n = 0; n < 256; n++) {
    let c = n
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    t[n] = c >>> 0
  }
  return t
})()

function crc32(buf: Uint8Array): number {
  let c = 0xffffffff
  for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff]! ^ (c >>> 8)
  return (c ^ 0xffffffff) >>> 0
}

function chunk(type: string, data: Uint8Array): Buffer {
  const typeBytes = Buffer.from(type, 'ascii')
  const body = Buffer.concat([typeBytes, Buffer.from(data)])
  const len = Buffer.alloc(4)
  len.writeUInt32BE(data.length)
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(body))
  return Buffer.concat([len, body, crc])
}

function encodePng(rgba: Uint8Array, size: number): Buffer {
  const ihdr = Buffer.alloc(13)
  ihdr.writeUInt32BE(size, 0)
  ihdr.writeUInt32BE(size, 4)
  ihdr[8] = 8 // 8 bits par canal
  ihdr[9] = 6 // RGBA
  // Compression, filtre et entrelacement restent aux valeurs par défaut (0).

  // Chaque scanline est préfixée de son octet de filtre ; 0 = aucun filtre.
  const raw = Buffer.alloc(size * (size * 4 + 1))
  for (let y = 0; y < size; y++) {
    raw[y * (size * 4 + 1)] = 0
    Buffer.from(rgba.buffer, y * size * 4, size * 4).copy(raw, y * (size * 4 + 1) + 1)
  }

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', new Uint8Array(0)),
  ])
}

// --- Dessin ------------------------------------------------------------------

function inside(p: Vec2, pts: Vec2[]): boolean {
  let hit = false
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const a = pts[i]!
    const b = pts[j]!
    if (a.y > p.y !== b.y > p.y && p.x < a.x + ((p.y - a.y) / (b.y - a.y)) * (b.x - a.x)) {
      hit = !hit
    }
  }
  return hit
}

function render(size: number): Uint8Array {
  const shape = { ...defaultShape(), preset: 'squircle' as const }
  shape.corners = { tl: 0.55, tr: 0.55, br: 0.55, bl: 0.55 }
  shape.smoothing = 1
  shape.inset = { top: 0.06, right: 0.06, bottom: 0.06, left: 0.06 }
  const pts = outline(shape, 64).map((p) => ({ x: p.x * size, y: p.y * size }))

  const rgba = new Uint8Array(size * size * 4)
  // 2×2 par pixel : suffisant pour lisser le bord aux tailles d'icône.
  const SUB = 2
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let hits = 0
      for (let sy = 0; sy < SUB; sy++) {
        for (let sx = 0; sx < SUB; sx++) {
          const p = { x: x + (sx + 0.5) / SUB, y: y + (sy + 0.5) / SUB }
          if (inside(p, pts)) hits++
        }
      }
      const cov = hits / (SUB * SUB)
      // Dégradé du bleu système iOS vers l'indigo, en diagonale.
      const t = (x / size + y / size) / 2
      const i = (y * size + x) * 4
      rgba[i] = Math.round(10 + t * 84)
      rgba[i + 1] = Math.round(132 - t * 40)
      rgba[i + 2] = Math.round(255 - t * 25)
      rgba[i + 3] = Math.round(cov * 255)
    }
  }
  return rgba
}

// --- ICO ---------------------------------------------------------------------

/** ICO embarquant des PNG : accepté depuis Windows Vista, et bien plus compact. */
function encodeIco(images: { size: number; png: Buffer }[]): Buffer {
  const header = Buffer.alloc(6)
  header.writeUInt16LE(0, 0) // réservé
  header.writeUInt16LE(1, 2) // type 1 = icône
  header.writeUInt16LE(images.length, 4)

  let offset = 6 + images.length * 16
  const entries: Buffer[] = []
  for (const { size, png } of images) {
    const e = Buffer.alloc(16)
    // 256 se code par 0 : le champ ne fait qu'un octet.
    e[0] = size >= 256 ? 0 : size
    e[1] = size >= 256 ? 0 : size
    e[2] = 0 // palette
    e[3] = 0 // réservé
    e.writeUInt16LE(1, 4) // plans
    e.writeUInt16LE(32, 6) // bits par pixel
    e.writeUInt32LE(png.length, 8)
    e.writeUInt32LE(offset, 12)
    entries.push(e)
    offset += png.length
  }

  return Buffer.concat([header, ...entries, ...images.map((i) => i.png)])
}

// --- Sortie ------------------------------------------------------------------

mkdirSync(outDir, { recursive: true })

const png = (size: number) => encodePng(render(size), size)

// Noms attendus par le bundler Tauri.
const named: [string, number][] = [
  ['32x32.png', 32],
  ['128x128.png', 128],
  ['128x128@2x.png', 256],
  ['icon.png', 512],
]
for (const [name, size] of named) {
  writeFileSync(join(outDir, name), png(size))
}

const icoSizes = [16, 32, 48, 256]
writeFileSync(
  join(outDir, 'icon.ico'),
  encodeIco(icoSizes.map((size) => ({ size, png: png(size) }))),
)

console.log(`icônes écrites dans ${outDir}`)
