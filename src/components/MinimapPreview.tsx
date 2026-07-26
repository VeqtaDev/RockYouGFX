/**
 * Aperçu temps réel de la minimap.
 *
 * Le fond est un faux plan généré procéduralement : aucune tuile Rockstar
 * n'est distribuée avec l'app. Si l'utilisateur désigne son install GTA V,
 * c'est la vraie tuile qui prendra sa place.
 */
import { useEffect, useRef } from 'react'
import { outline, traceOutline, type MinimapShape } from '../lib/shape'
import { lerpTowards, shapeToVector, vectorToShape } from '../lib/animate'

export function MinimapPreview({ shape }: { shape: MinimapShape }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const target = useRef(shape)
  const current = useRef(shapeToVector(shape))
  const velocity = useRef(new Float64Array(shapeToVector(shape).length))

  target.current = shape

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    let raf = 0
    let last = performance.now()

    const frame = (now: number) => {
      // Un onglet en arrière-plan produit des dt énormes : les borner évite
      // que le ressort n'explose au retour au premier plan.
      const dt = Math.min(0.05, (now - last) / 1000)
      last = now

      const goal = shapeToVector(target.current)
      lerpTowards(current.current, velocity.current, goal, dt)
      draw(ctx, canvas, vectorToShape(current.current, target.current))

      raf = requestAnimationFrame(frame)
    }
    raf = requestAnimationFrame(frame)
    return () => cancelAnimationFrame(raf)
  }, [])

  return (
    <canvas
      ref={canvasRef}
      className="h-full w-full"
      // Le canvas est redimensionné par le code au ratio de l'écran ;
      // ces valeurs ne sont qu'un point de départ avant le premier layout.
      width={512}
      height={512}
    />
  )
}

function draw(ctx: CanvasRenderingContext2D, canvas: HTMLCanvasElement, shape: MinimapShape) {
  const dpr = window.devicePixelRatio || 1
  const cssW = canvas.clientWidth || 512
  const cssH = canvas.clientHeight || 512
  if (canvas.width !== Math.round(cssW * dpr) || canvas.height !== Math.round(cssH * dpr)) {
    canvas.width = Math.round(cssW * dpr)
    canvas.height = Math.round(cssH * dpr)
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, cssW, cssH)

  // La minimap GTA V est plus large que haute ; on garde ce ratio dans la
  // zone de dessin pour que l'aperçu ne mente pas sur la déformation.
  const aspect = 4 / 3
  // Les barres de vie/armure sont dessinées *sous* le contour : sans cette
  // réserve elles tombent hors du canvas et deviennent invisibles.
  const HUD_RESERVE = 28
  const availW = cssW * 0.86
  const availH = (cssH - HUD_RESERVE) * 0.92
  let w = availW
  let h = w / aspect
  if (h > availH) {
    h = availH
    w = h * aspect
  }
  const ox = (cssW - w) / 2
  const oy = (cssH - HUD_RESERVE - h) / 2

  const pts = outline(shape, 48)
  const path = new Path2D()
  traceOutline(path, pts, w, h, ox, oy)

  ctx.save()
  ctx.clip(path)
  drawFakeMap(ctx, ox, oy, w, h)
  ctx.restore()

  if (shape.border.visible && shape.border.width > 0) {
    ctx.save()
    ctx.strokeStyle = shape.border.color
    // Le trait est centré sur le chemin : la moitié déborde. On le double et
    // on reclippe pour que la bordure reste strictement à l'intérieur.
    ctx.lineWidth = shape.border.width * 2
    ctx.clip(path)
    ctx.stroke(path)
    ctx.restore()
  }

  drawHud(ctx, shape, pts, ox, oy, w, h)
  drawPlayerBlip(ctx, ox + w / 2, oy + h / 2)
}

/**
 * Faux plan : trame de rues, îlots bâtis, plan d'eau et une voie rapide.
 * Entièrement déterministe — l'aperçu ne doit pas scintiller d'une frame
 * à l'autre.
 */
function drawFakeMap(ctx: CanvasRenderingContext2D, ox: number, oy: number, w: number, h: number) {
  // Générateur congruentiel : même plan à chaque rendu, sans dépendance.
  let seed = 20250726
  const rnd = () => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff)

  ctx.fillStyle = '#2a2f3a'
  ctx.fillRect(ox, oy, w, h)

  // Baie en bas à droite.
  ctx.fillStyle = '#16283d'
  ctx.beginPath()
  ctx.moveTo(ox + w, oy + h * 0.42)
  ctx.quadraticCurveTo(ox + w * 0.72, oy + h * 0.62, ox + w * 0.78, oy + h)
  ctx.lineTo(ox + w, oy + h)
  ctx.closePath()
  ctx.fill()

  // Îlots bâtis sur une trame régulière : c'est la régularité qui fait
  // « plan de ville » plutôt que « gribouillis ».
  const cols = 7
  const rows = 5
  const cw = w / cols
  const ch = h / rows
  for (let cxi = 0; cxi < cols; cxi++) {
    for (let cyi = 0; cyi < rows; cyi++) {
      if (cxi / cols > 0.72 && cyi / rows > 0.5) continue // dans l'eau
      const pad = Math.min(cw, ch) * 0.16
      const jx = (rnd() - 0.5) * pad
      const jy = (rnd() - 0.5) * pad
      ctx.fillStyle = rnd() > 0.72 ? '#39404d' : '#333944'
      ctx.fillRect(ox + cxi * cw + pad + jx, oy + cyi * ch + pad + jy, cw - pad * 2, ch - pad * 2)
    }
  }

  // Rues : la trame qui sépare les îlots.
  ctx.strokeStyle = '#4d5666'
  ctx.lineWidth = Math.max(1, w * 0.004)
  for (let i = 1; i < cols; i++) {
    ctx.beginPath()
    ctx.moveTo(ox + i * cw, oy)
    ctx.lineTo(ox + i * cw, oy + h)
    ctx.stroke()
  }
  for (let i = 1; i < rows; i++) {
    ctx.beginPath()
    ctx.moveTo(ox, oy + i * ch)
    ctx.lineTo(ox + w, oy + i * ch)
    ctx.stroke()
  }

  // Voie rapide en diagonale.
  ctx.strokeStyle = '#c8a33c'
  ctx.lineWidth = Math.max(2, w * 0.011)
  ctx.lineJoin = 'round'
  ctx.beginPath()
  ctx.moveTo(ox, oy + h * 0.78)
  ctx.lineTo(ox + w * 0.38, oy + h * 0.52)
  ctx.lineTo(ox + w * 0.66, oy + h * 0.55)
  ctx.lineTo(ox + w, oy + h * 0.22)
  ctx.stroke()
}

/**
 * Barres de vie et d'armure.
 *
 * `follow` les colle au bas du contour réel plutôt qu'à la boîte englobante :
 * c'est toute la différence entre une bordure ronde propre et des barres qui
 * flottent dans le vide.
 */
function drawHud(
  ctx: CanvasRenderingContext2D,
  shape: MinimapShape,
  pts: { x: number; y: number }[],
  ox: number,
  oy: number,
  w: number,
  h: number,
) {
  const bars: { mode: string; color: string; frac: number }[] = [
    { mode: shape.hud.health, color: '#30d158', frac: 0.72 },
    { mode: shape.hud.armour, color: '#64d2ff', frac: 0.5 },
  ]

  let minY = Infinity
  let maxY = -Infinity
  for (const p of pts) {
    if (p.y < minY) minY = p.y
    if (p.y > maxY) maxY = p.y
  }
  const bounds = { minY, maxY }

  let slot = 0
  for (const bar of bars) {
    if (bar.mode === 'hidden') continue

    let left: number
    let right: number
    let y: number

    if (bar.mode === 'vanilla') {
      // Position d'origine : calée sur la boîte, elle ignore la forme.
      left = ox
      right = ox + w
      y = oy + h + 6 + slot * 7
    } else {
      // `follow` doit lire la largeur *du contour*, donc se placer à
      // l'intérieur de la forme. Les positions sont dérivées de l'extension
      // verticale réelle du contour : une fraction fixe tomberait sous le bas
      // d'un cercle ou d'un hexagone, et la barre disparaîtrait.
      const ny = bounds.maxY - (0.13 - slot * 0.06) * (bounds.maxY - bounds.minY)
      y = oy + h * ny
      const span = spanAtY(pts, ny)
      if (!span) continue
      // Léger retrait pour que la barre ne morde pas la bordure.
      const pad = (span[1] - span[0]) * 0.06
      left = ox + (span[0] + pad) * w
      right = ox + (span[1] - pad) * w
    }

    const width = right - left
    if (width <= 0) continue

    ctx.fillStyle = 'rgba(255,255,255,0.18)'
    roundRect(ctx, left, y, width, 4, 2)
    ctx.fill()
    ctx.fillStyle = bar.color
    roundRect(ctx, left, y, width * bar.frac, 4, 2)
    ctx.fill()
    slot++
  }
}

/** Intervalle horizontal [min,max] du contour à une hauteur normalisée donnée. */
function spanAtY(pts: { x: number; y: number }[], ny: number): [number, number] | null {
  let lo = Infinity
  let hi = -Infinity
  for (let i = 0; i < pts.length; i++) {
    const a = pts[i]!
    const b = pts[(i + 1) % pts.length]!
    if (a.y === b.y) continue
    const t = (ny - a.y) / (b.y - a.y)
    if (t < 0 || t > 1) continue
    const x = a.x + (b.x - a.x) * t
    if (x < lo) lo = x
    if (x > hi) hi = x
  }
  return lo <= hi ? [lo, hi] : null
}

function drawPlayerBlip(ctx: CanvasRenderingContext2D, cx: number, cy: number) {
  ctx.save()
  ctx.translate(cx, cy)
  ctx.fillStyle = '#ffffff'
  ctx.strokeStyle = 'rgba(0,0,0,0.6)'
  ctx.lineWidth = 1.5
  ctx.beginPath()
  ctx.moveTo(0, -9)
  ctx.lineTo(6, 7)
  ctx.lineTo(0, 3.5)
  ctx.lineTo(-6, 7)
  ctx.closePath()
  ctx.fill()
  ctx.stroke()
  ctx.restore()
}

function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  ctx.beginPath()
  ctx.roundRect(x, y, w, h, r)
}
