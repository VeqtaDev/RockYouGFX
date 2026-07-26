/**
 * Primitives d'interface iOS.
 *
 * Les ressorts sont centralisés dans `spring` : sur iOS, la cohérence du
 * timing entre composants fait plus pour la sensation « native » que le
 * détail de chaque composant pris isolément.
 */
import { motion } from 'motion/react'
import { useCallback, useId, useRef, useState, type ReactNode } from 'react'

export const spring = {
  /** Réponse immédiate : segmented control, interrupteurs. */
  snappy: { type: 'spring', stiffness: 520, damping: 34, mass: 0.7 },
  /** Déplacements de contenu, morphs de forme. */
  smooth: { type: 'spring', stiffness: 260, damping: 30 },
  /** Feuilles et gros panneaux. */
  sheet: { type: 'spring', stiffness: 320, damping: 34, mass: 0.9 },
} as const

const cx = (...parts: (string | false | null | undefined)[]) => parts.filter(Boolean).join(' ')

// --- Listes groupées --------------------------------------------------------

export function Section({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <section className="mb-6">
      {title && (
        <h2 className="text-caption mb-2 px-4 text-label-2 uppercase">{title}</h2>
      )}
      <div className="overflow-hidden rounded-ios-lg bg-bg-raised">{children}</div>
    </section>
  )
}

export function Row({
  label,
  detail,
  children,
  stacked = false,
}: {
  label: string
  detail?: string
  children?: ReactNode
  stacked?: boolean
}) {
  return (
    <div
      className={cx(
        'px-4 py-3',
        // Le séparateur iOS s'arrête avant le bord gauche, aligné sur le texte.
        'border-b border-separator last:border-b-0',
        !stacked && 'flex items-center justify-between gap-4',
      )}
    >
      <div className={cx('min-w-0', stacked && 'mb-3')}>
        <div className="text-body text-label">{label}</div>
        {detail && <div className="text-footnote mt-0.5 text-label-2">{detail}</div>}
      </div>
      {children}
    </div>
  )
}

// --- Segmented control ------------------------------------------------------

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  columns,
}: {
  value: T
  options: { value: T; label: string }[]
  onChange: (v: T) => void
  /** Passe sur plusieurs rangées : au-delà de 4 segments, une seule ligne tronque les libellés. */
  columns?: number
}) {
  // layoutId fait glisser la pastille d'un segment à l'autre au lieu
  // de la faire disparaître/réapparaître.
  const groupId = useId()
  return (
    <div
      role="tablist"
      className="grid gap-0.5 rounded-ios-sm bg-fill-4 p-0.5"
      style={{ gridTemplateColumns: `repeat(${columns ?? options.length}, minmax(0,1fr))` }}
    >
      {options.map((o) => {
        const active = o.value === value
        return (
          <button
            key={o.value}
            role="tab"
            aria-selected={active}
            onClick={() => onChange(o.value)}
            className="relative flex-1 whitespace-nowrap px-3 py-1.5"
          >
            {active && (
              <motion.span
                layoutId={`seg-${groupId}`}
                transition={spring.snappy}
                className="absolute inset-0 rounded-[6px] bg-bg-quaternary shadow-sm"
              />
            )}
            <span
              className={cx(
                'text-footnote relative z-10 transition-colors',
                active ? 'font-semibold text-label' : 'text-label-2',
              )}
            >
              {o.label}
            </span>
          </button>
        )
      })}
    </div>
  )
}

// --- Interrupteur -----------------------------------------------------------

export function Switch({
  checked,
  onChange,
}: {
  checked: boolean
  onChange: (v: boolean) => void
}) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={cx(
        'relative h-[31px] w-[51px] shrink-0 rounded-full transition-colors duration-200',
        checked ? 'bg-ios-green' : 'bg-fill-1',
      )}
    >
      <motion.span
        layout
        transition={spring.snappy}
        className="absolute top-[2px] h-[27px] w-[27px] rounded-full bg-white shadow-md"
        style={{ left: checked ? 22 : 2 }}
      />
    </button>
  )
}

// --- Slider -----------------------------------------------------------------

export function Slider({
  value,
  min = 0,
  max = 1,
  step = 0.01,
  onChange,
  format,
}: {
  value: number
  min?: number
  max?: number
  step?: number
  onChange: (v: number) => void
  format?: (v: number) => string
}) {
  const trackRef = useRef<HTMLDivElement>(null)
  const [dragging, setDragging] = useState(false)
  const pct = max === min ? 0 : (value - min) / (max - min)

  const quantize = useCallback(
    (raw: number) => {
      const snapped = Math.round(raw / step) * step
      // Le pas flottant traîne des erreurs de représentation : on arrondit
      // au nombre de décimales impliqué par `step`.
      const decimals = Math.max(0, Math.ceil(-Math.log10(step)))
      return Number(Math.min(max, Math.max(min, snapped)).toFixed(decimals))
    },
    [step, min, max],
  )

  const setFromClientX = useCallback(
    (clientX: number) => {
      const el = trackRef.current
      if (!el) return
      const r = el.getBoundingClientRect()
      if (r.width === 0) return
      const t = Math.min(1, Math.max(0, (clientX - r.left) / r.width))
      onChange(quantize(min + t * (max - min)))
    },
    [min, max, onChange, quantize],
  )

  const onKeyDown = (e: React.KeyboardEvent) => {
    const big = (max - min) / 10
    const delta =
      e.key === 'ArrowRight' || e.key === 'ArrowUp'
        ? step
        : e.key === 'ArrowLeft' || e.key === 'ArrowDown'
          ? -step
          : e.key === 'PageUp'
            ? big
            : e.key === 'PageDown'
              ? -big
              : 0
    if (delta === 0) return
    e.preventDefault()
    onChange(quantize(value + delta))
  }

  return (
    <div className="flex items-center gap-3">
      <div
        ref={trackRef}
        role="slider"
        tabIndex={0}
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={value}
        onKeyDown={onKeyDown}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId)
          setDragging(true)
          setFromClientX(e.clientX)
        }}
        onPointerMove={(e) => dragging && setFromClientX(e.clientX)}
        onPointerUp={(e) => {
          e.currentTarget.releasePointerCapture(e.pointerId)
          setDragging(false)
        }}
        className="relative h-7 flex-1 cursor-pointer touch-none"
      >
        <div className="absolute top-1/2 h-1 w-full -translate-y-1/2 rounded-full bg-fill-3">
          <div
            className="h-full rounded-full bg-ios-blue"
            style={{ width: `${pct * 100}%` }}
          />
        </div>
        <motion.div
          // Le pouce grossit sous le doigt : retour tactile visuel d'iOS.
          animate={{ scale: dragging ? 1.15 : 1 }}
          transition={spring.snappy}
          className="absolute top-1/2 h-[22px] w-[22px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-white shadow-lg"
          style={{ left: `${pct * 100}%` }}
        />
      </div>
      {format && (
        <span className="text-footnote w-14 shrink-0 text-right font-mono tabular-nums text-label-2">
          {format(value)}
        </span>
      )}
    </div>
  )
}

// --- Bouton -----------------------------------------------------------------

export function Button({
  children,
  onClick,
  variant = 'plain',
  disabled,
}: {
  children: ReactNode
  onClick?: () => void
  variant?: 'filled' | 'tinted' | 'plain'
  disabled?: boolean
}) {
  return (
    <motion.button
      whileTap={disabled ? undefined : { scale: 0.96 }}
      transition={spring.snappy}
      onClick={onClick}
      disabled={disabled}
      className={cx(
        'text-headline rounded-ios px-4 py-2.5 transition-opacity',
        disabled && 'opacity-40',
        variant === 'filled' && 'bg-ios-blue text-white',
        variant === 'tinted' && 'bg-ios-blue/15 text-ios-blue',
        variant === 'plain' && 'text-ios-blue',
      )}
    >
      {children}
    </motion.button>
  )
}
