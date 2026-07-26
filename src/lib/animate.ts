/**
 * Morphing de forme par ressort.
 *
 * La forme est aplatie en vecteur de nombres, intégrée par un ressort, puis
 * reconstruite. Interpoler les *paramètres* plutôt que les points du contour
 * est indispensable : deux contours n'ont pas forcément le même nombre de
 * points (un coin de rayon nul en produit un seul, un coin arrondi 49).
 */
import type { MinimapShape } from './shape'

const STIFFNESS = 240
const DAMPING = 26
/** Pas d'intégration fixe : le ressort reste stable quelle que soit la cadence d'affichage. */
const SUBSTEP = 1 / 120

export function shapeToVector(s: MinimapShape): Float64Array {
  return Float64Array.from([
    s.inset.top,
    s.inset.right,
    s.inset.bottom,
    s.inset.left,
    s.corners.tl,
    s.corners.tr,
    s.corners.br,
    s.corners.bl,
    s.smoothing,
    s.feather,
    s.polygon.sides,
    s.polygon.rotation,
    s.border.width,
  ])
}

/** `ref` fournit les champs non numériques (preset, couleurs, modes HUD). */
export function vectorToShape(v: Float64Array, ref: MinimapShape): MinimapShape {
  return {
    preset: ref.preset,
    inset: { top: v[0]!, right: v[1]!, bottom: v[2]!, left: v[3]! },
    corners: { tl: v[4]!, tr: v[5]!, br: v[6]!, bl: v[7]! },
    smoothing: v[8]!,
    feather: v[9]!,
    // Un nombre de côtés fractionnaire n'a pas de sens géométrique.
    polygon: { sides: Math.round(v[10]!), rotation: v[11]! },
    border: { ...ref.border, width: v[12]! },
    hud: ref.hud,
  }
}

export function lerpTowards(
  x: Float64Array,
  v: Float64Array,
  goal: Float64Array,
  dt: number,
): void {
  let remaining = dt
  while (remaining > 0) {
    const step = Math.min(SUBSTEP, remaining)
    remaining -= step
    for (let i = 0; i < x.length; i++) {
      const accel = -STIFFNESS * (x[i]! - goal[i]!) - DAMPING * v[i]!
      v[i] = v[i]! + accel * step
      x[i] = x[i]! + v[i]! * step
    }
  }
}
