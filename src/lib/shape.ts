/**
 * Modèle de forme de la minimap — source de vérité unique du projet.
 *
 * Ce fichier a un miroir strict côté Rust dans `crates/core/src/shape.rs`.
 * Toute évolution ici doit y être répercutée, et le test de conformité
 * `crates/core/tests/parity.rs` compare les deux implémentations.
 *
 * Invariant central : **tout est exprimé en fractions, jamais en pixels.**
 * La même définition alimente `radarmasksm.dds` et `radarmasklg.dds`, qui
 * n'ont pas les mêmes dimensions. Un rayon en pixels donnerait deux formes
 * différentes selon la texture.
 */

export type Preset = 'vanilla' | 'rounded' | 'circle' | 'squircle' | 'polygon'

/** `follow` = l'élément épouse la nouvelle forme au lieu de rester en position vanilla. */
export type HudMode = 'vanilla' | 'follow' | 'hidden'

export interface Vec2 {
  x: number
  y: number
}

export interface Corners {
  tl: number
  tr: number
  br: number
  bl: number
}

export interface Inset {
  top: number
  right: number
  bottom: number
  left: number
}

export interface MinimapShape {
  preset: Preset
  /** Marges depuis les bords de la texture, en fraction 0..1. */
  inset: Inset
  /** Rayon par coin, en fraction 0..1 de la demi-plus-petite-dimension de la boîte. */
  corners: Corners
  /** 0 = arc de cercle exact · 1 = squircle façon iOS. Pilote l'exposant de la superellipse. */
  smoothing: number
  /** Adoucissement du bord du masque, en fraction de la plus petite dimension de la boîte. */
  feather: number
  /** Uniquement pour le preset `polygon`. */
  polygon: { sides: number; rotation: number }
  border: { visible: boolean; width: number; color: string }
  hud: { health: HudMode; armour: HudMode; compass: HudMode }
}

// --- Bornes -----------------------------------------------------------------

export const SMOOTHING_MIN_EXPONENT = 2 // superellipse d'exposant 2 = cercle parfait
export const SMOOTHING_MAX_EXPONENT = 5 // ~ le squircle des icônes iOS
export const POLYGON_MIN_SIDES = 3
export const POLYGON_MAX_SIDES = 12

const clamp = (v: number, lo: number, hi: number) => (v < lo ? lo : v > hi ? hi : v)
const clamp01 = (v: number) => clamp(v, 0, 1)

// --- Presets ----------------------------------------------------------------

export function defaultShape(): MinimapShape {
  return {
    preset: 'rounded',
    inset: { top: 0.02, right: 0.02, bottom: 0.02, left: 0.02 },
    corners: { tl: 0.22, tr: 0.22, br: 0.22, bl: 0.22 },
    smoothing: 0.6,
    feather: 0.004,
    polygon: { sides: 6, rotation: 0 },
    border: { visible: true, width: 2, color: '#000000' },
    hud: { health: 'follow', armour: 'follow', compass: 'vanilla' },
  }
}

/** Un preset n'est qu'un jeu de paramètres : rien n'est verrouillé après application. */
export function applyPreset(shape: MinimapShape, preset: Preset): MinimapShape {
  const base = { ...shape, preset }
  switch (preset) {
    case 'vanilla':
      return { ...base, corners: { tl: 0, tr: 0, br: 0, bl: 0 }, smoothing: 0 }
    case 'rounded':
      return { ...base, corners: { tl: 0.22, tr: 0.22, br: 0.22, bl: 0.22 }, smoothing: 0.6 }
    case 'circle':
      return { ...base, corners: { tl: 1, tr: 1, br: 1, bl: 1 }, smoothing: 0 }
    case 'squircle':
      return { ...base, corners: { tl: 1, tr: 1, br: 1, bl: 1 }, smoothing: 1 }
    case 'polygon':
      // Sans rayon propre, le polygone hériterait de celui du preset
      // précédent : à 100 % un hexagone s'arrondit jusqu'à devenir un disque.
      return { ...base, corners: { tl: 0.12, tr: 0.12, br: 0.12, bl: 0.12 } }
  }
}

// --- Géométrie --------------------------------------------------------------

interface Box {
  x0: number
  y0: number
  x1: number
  y1: number
}

function boxOf(shape: MinimapShape): Box {
  const { top, right, bottom, left } = shape.inset
  // Des marges opposées qui se croisent donneraient une boîte inversée.
  const x0 = clamp01(left)
  const x1 = clamp(1 - right, x0, 1)
  const y0 = clamp01(top)
  const y1 = clamp(1 - bottom, y0, 1)
  return { x0, y0, x1, y1 }
}

/**
 * Rayons en unités absolues, avec la mise à l'échelle de CSS `border-radius` :
 * si deux rayons voisins dépassent la longueur de leur arête commune, tous les
 * rayons sont réduits du même facteur. Sans ça, les coins se chevauchent et
 * l'outline se replie sur elle-même.
 */
function resolveRadii(shape: MinimapShape, box: Box): Corners {
  const w = box.x1 - box.x0
  const h = box.y1 - box.y0
  const maxR = Math.min(w, h) / 2

  let tl = clamp01(shape.corners.tl) * maxR
  let tr = clamp01(shape.corners.tr) * maxR
  let br = clamp01(shape.corners.br) * maxR
  let bl = clamp01(shape.corners.bl) * maxR

  const ratios = [
    tl + tr > 0 ? w / (tl + tr) : Infinity, // arête haute
    br + bl > 0 ? w / (br + bl) : Infinity, // arête basse
    tl + bl > 0 ? h / (tl + bl) : Infinity, // arête gauche
    tr + br > 0 ? h / (tr + br) : Infinity, // arête droite
  ]
  const f = Math.min(1, ...ratios)
  if (f < 1) {
    tl *= f
    tr *= f
    br *= f
    bl *= f
  }
  return { tl, tr, br, bl }
}

function exponent(smoothing: number): number {
  return (
    SMOOTHING_MIN_EXPONENT +
    clamp01(smoothing) * (SMOOTHING_MAX_EXPONENT - SMOOTHING_MIN_EXPONENT)
  )
}

/**
 * Échantillonne un quadrant de superellipse |x/r|^n + |y/r|^n = 1.
 *
 * n = 2 redonne exactement le cercle, donc `smoothing = 0` produit un coin
 * arrondi classique et il n'y a pas deux chemins de code à maintenir.
 */
function superellipseQuadrant(n: number, steps: number): Vec2[] {
  const p = 2 / n
  const out: Vec2[] = []
  for (let i = 0; i <= steps; i++) {
    const t = (i / steps) * (Math.PI / 2)
    out.push({ x: Math.pow(Math.cos(t), p), y: Math.pow(Math.sin(t), p) })
  }
  return out
}

/**
 * Contour de la forme, en coordonnées normalisées [0,1]² (y vers le bas),
 * dans le sens horaire.
 *
 * Le rasterizer Rust et l'aperçu Canvas consomment tous les deux cette
 * fonction (via son miroir) : c'est ce qui garantit que ce qu'on voit à
 * l'écran est ce qui finit dans le DDS.
 */
export function outline(shape: MinimapShape, samplesPerCorner = 48): Vec2[] {
  if (shape.preset === 'polygon') return polygonOutline(shape, samplesPerCorner)

  const box = boxOf(shape)
  const r = resolveRadii(shape, box)
  const n = exponent(shape.smoothing)
  const q = superellipseQuadrant(n, samplesPerCorner)
  const pts: Vec2[] = []

  // Coin haut-gauche : arête gauche -> arête haute.
  const tlx = box.x0 + r.tl
  const tly = box.y0 + r.tl
  if (r.tl === 0) pts.push({ x: box.x0, y: box.y0 })
  else for (const s of q) pts.push({ x: tlx - r.tl * s.x, y: tly - r.tl * s.y })

  // Coin haut-droit : arête haute -> arête droite.
  const trx = box.x1 - r.tr
  const try_ = box.y0 + r.tr
  if (r.tr === 0) pts.push({ x: box.x1, y: box.y0 })
  else for (const s of q) pts.push({ x: trx + r.tr * s.y, y: try_ - r.tr * s.x })

  // Coin bas-droit : arête droite -> arête basse.
  const brx = box.x1 - r.br
  const bry = box.y1 - r.br
  if (r.br === 0) pts.push({ x: box.x1, y: box.y1 })
  else for (const s of q) pts.push({ x: brx + r.br * s.x, y: bry + r.br * s.y })

  // Coin bas-gauche : arête basse -> arête gauche.
  const blx = box.x0 + r.bl
  const bly = box.y1 - r.bl
  if (r.bl === 0) pts.push({ x: box.x0, y: box.y1 })
  else for (const s of q) pts.push({ x: blx - r.bl * s.y, y: bly + r.bl * s.x })

  return pts
}

/**
 * N-gone inscrit dans la boîte, coins adoucis par un arc de cercle.
 *
 * Le lissage superellipse ne s'applique pas ici : il est défini pour un coin
 * à 90°, pas pour un angle quelconque. Les polygones utilisent donc un arc
 * circulaire, quelle que soit la valeur de `smoothing`.
 */
function polygonOutline(shape: MinimapShape, samplesPerCorner: number): Vec2[] {
  const box = boxOf(shape)
  const cx = (box.x0 + box.x1) / 2
  const cy = (box.y0 + box.y1) / 2
  const rx = (box.x1 - box.x0) / 2
  const ry = (box.y1 - box.y0) / 2

  const sides = Math.round(clamp(shape.polygon.sides, POLYGON_MIN_SIDES, POLYGON_MAX_SIDES))
  const rot = shape.polygon.rotation

  const verts: Vec2[] = []
  for (let i = 0; i < sides; i++) {
    // -PI/2 place un sommet en haut, ce qui correspond à l'attente visuelle.
    const a = rot + (i / sides) * Math.PI * 2 - Math.PI / 2
    verts.push({ x: cx + rx * Math.cos(a), y: cy + ry * Math.sin(a) })
  }

  const radiusFrac = clamp01((shape.corners.tl + shape.corners.tr + shape.corners.br + shape.corners.bl) / 4)
  if (radiusFrac <= 0) return verts

  const out: Vec2[] = []
  for (let i = 0; i < sides; i++) {
    const v = verts[i]!
    const prev = verts[(i - 1 + sides) % sides]!
    const next = verts[(i + 1) % sides]!

    const da = norm(sub(prev, v))
    const db = norm(sub(next, v))
    const dot = clamp(da.x * db.x + da.y * db.y, -1, 1)
    const theta = Math.acos(dot)

    // Sommets colinéaires : pas de coin à arrondir.
    if (theta < 1e-6 || Math.PI - theta < 1e-6) {
      out.push(v)
      continue
    }

    const halfPrev = len(sub(prev, v)) / 2
    const halfNext = len(sub(next, v)) / 2
    const maxTan = Math.min(halfPrev, halfNext)
    const tan = radiusFrac * maxTan
    // Relation tangente/rayon dans un coin d'angle intérieur theta.
    const r = tan * Math.tan(theta / 2)

    const p1 = { x: v.x + da.x * tan, y: v.y + da.y * tan }
    const p2 = { x: v.x + db.x * tan, y: v.y + db.y * tan }
    const bis = norm({ x: da.x + db.x, y: da.y + db.y })
    const dist = r / Math.sin(theta / 2)
    const c = { x: v.x + bis.x * dist, y: v.y + bis.y * dist }

    const a1 = Math.atan2(p1.y - c.y, p1.x - c.x)
    const a2 = Math.atan2(p2.y - c.y, p2.x - c.x)
    let sweep = a2 - a1
    while (sweep > Math.PI) sweep -= Math.PI * 2
    while (sweep < -Math.PI) sweep += Math.PI * 2

    for (let s = 0; s <= samplesPerCorner; s++) {
      const a = a1 + (sweep * s) / samplesPerCorner
      out.push({ x: c.x + r * Math.cos(a), y: c.y + r * Math.sin(a) })
    }
  }
  return out
}

const sub = (a: Vec2, b: Vec2): Vec2 => ({ x: a.x - b.x, y: a.y - b.y })
const len = (a: Vec2): number => Math.hypot(a.x, a.y)
function norm(a: Vec2): Vec2 {
  const l = len(a)
  return l === 0 ? { x: 0, y: 0 } : { x: a.x / l, y: a.y / l }
}

/** Trace le contour dans un contexte Canvas, mis à l'échelle d'une boîte w×h. */
export function traceOutline(
  ctx: CanvasRenderingContext2D | Path2D,
  pts: Vec2[],
  w: number,
  h: number,
  ox = 0,
  oy = 0,
): void {
  if (pts.length === 0) return
  const first = pts[0]!
  ctx.moveTo(ox + first.x * w, oy + first.y * h)
  for (let i = 1; i < pts.length; i++) {
    const p = pts[i]!
    ctx.lineTo(ox + p.x * w, oy + p.y * h)
  }
  ctx.closePath()
}
