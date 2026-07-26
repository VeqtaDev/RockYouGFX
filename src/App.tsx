import { AnimatePresence, motion } from 'motion/react'
import { useState } from 'react'
import { MinimapPreview } from './components/MinimapPreview'
import { UpdateBanner } from './components/UpdateBanner'
import { Button, Row, Section, Segmented, Slider, Switch, spring } from './components/ui'
import {
  POLYGON_MAX_SIDES,
  POLYGON_MIN_SIDES,
  applyPreset,
  defaultShape,
  type HudMode,
  type MinimapShape,
  type Preset,
} from './lib/shape'

const PRESETS: { value: Preset; label: string }[] = [
  { value: 'vanilla', label: 'Vanilla' },
  { value: 'rounded', label: 'Arrondi' },
  { value: 'circle', label: 'Cercle' },
  { value: 'squircle', label: 'Squircle' },
  { value: 'polygon', label: 'Polygone' },
]

const HUD_MODES: { value: HudMode; label: string }[] = [
  { value: 'vanilla', label: 'Vanilla' },
  { value: 'follow', label: 'Suit' },
  { value: 'hidden', label: 'Masqué' },
]

const pct = (v: number) => `${Math.round(v * 100)}%`

export default function App() {
  const [shape, setShape] = useState<MinimapShape>(defaultShape)
  const [linkCorners, setLinkCorners] = useState(true)

  const patch = (p: Partial<MinimapShape>) => setShape((s) => ({ ...s, ...p }))

  const setCorner = (key: keyof MinimapShape['corners'], v: number) =>
    setShape((s) => ({
      ...s,
      corners: linkCorners
        ? { tl: v, tr: v, br: v, bl: v }
        : { ...s.corners, [key]: v },
    }))

  const setInset = (v: number) => patch({ inset: { top: v, right: v, bottom: v, left: v } })

  return (
    <div className="flex h-full">
      {/* Aperçu */}
      <main className="relative flex-1 overflow-hidden bg-bg-base">
        <UpdateBanner />
        <div
          className="absolute inset-0 opacity-40"
          style={{
            background:
              'radial-gradient(120% 90% at 50% 0%, rgba(10,132,255,0.16), transparent 60%)',
          }}
        />
        <header className="relative px-10 pt-10">
          <h1 className="text-title-lg">RockYouGFX</h1>
          <p className="text-body mt-1 text-label-2">
            Éditeur de minimap GTA&nbsp;V pour FiveM
          </p>
        </header>
        <div className="absolute inset-x-0 bottom-0 top-32 p-10">
          <MinimapPreview shape={shape} />
        </div>
      </main>

      {/* Inspecteur */}
      <aside className="material-thick w-[380px] shrink-0 overflow-y-auto border-l border-separator px-4 py-6">
        <Section title="Forme">
          <Row label="Preset" stacked>
            <Segmented
              value={shape.preset}
              options={PRESETS}
              columns={3}
              onChange={(p) => setShape((s) => applyPreset(s, p))}
            />
          </Row>

          <AnimatePresence initial={false}>
            {shape.preset === 'polygon' && (
              <motion.div
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: 'auto', opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                transition={spring.smooth}
                className="overflow-hidden"
              >
                <Row label="Côtés" stacked>
                  <Slider
                    value={shape.polygon.sides}
                    min={POLYGON_MIN_SIDES}
                    max={POLYGON_MAX_SIDES}
                    step={1}
                    onChange={(v) => patch({ polygon: { ...shape.polygon, sides: v } })}
                    format={(v) => String(v)}
                  />
                </Row>
                <Row label="Rotation" stacked>
                  <Slider
                    value={shape.polygon.rotation}
                    min={0}
                    max={Math.PI * 2}
                    step={0.01}
                    onChange={(v) => patch({ polygon: { ...shape.polygon, rotation: v } })}
                    format={(v) => `${Math.round((v * 180) / Math.PI)}°`}
                  />
                </Row>
              </motion.div>
            )}
          </AnimatePresence>

          <Row label="Coins liés" detail="Un seul rayon pour les quatre coins">
            <Switch checked={linkCorners} onChange={setLinkCorners} />
          </Row>

          {linkCorners ? (
            <Row label="Rayon" stacked>
              <Slider
                value={shape.corners.tl}
                onChange={(v) => setCorner('tl', v)}
                format={pct}
              />
            </Row>
          ) : (
            (['tl', 'tr', 'br', 'bl'] as const).map((k) => (
              <Row
                key={k}
                label={
                  { tl: 'Haut gauche', tr: 'Haut droit', br: 'Bas droit', bl: 'Bas gauche' }[k]
                }
                stacked
              >
                <Slider
                  value={shape.corners[k]}
                  onChange={(v) => setCorner(k, v)}
                  format={pct}
                />
              </Row>
            ))
          )}

          <Row
            label="Lissage"
            detail="0 = arc de cercle · 100 % = squircle iOS"
            stacked
          >
            <Slider
              value={shape.smoothing}
              onChange={(v) => patch({ smoothing: v })}
              format={pct}
            />
          </Row>
        </Section>

        <Section title="Masque">
          <Row label="Marge" detail="Retrait depuis les bords de la texture" stacked>
            <Slider
              value={shape.inset.top}
              max={0.3}
              onChange={setInset}
              format={pct}
            />
          </Row>
          <Row label="Adoucissement" detail="Dégradé alpha sur le bord" stacked>
            <Slider
              value={shape.feather}
              max={0.05}
              step={0.001}
              onChange={(v) => patch({ feather: v })}
              format={(v) => `${(v * 100).toFixed(1)}%`}
            />
          </Row>
        </Section>

        <Section title="Bordure">
          <Row label="Afficher">
            <Switch
              checked={shape.border.visible}
              onChange={(v) => patch({ border: { ...shape.border, visible: v } })}
            />
          </Row>
          {shape.border.visible && (
            <Row label="Épaisseur" stacked>
              <Slider
                value={shape.border.width}
                min={0}
                max={8}
                step={0.5}
                onChange={(v) => patch({ border: { ...shape.border, width: v } })}
                format={(v) => `${v}px`}
              />
            </Row>
          )}
        </Section>

        <Section title="HUD">
          {(
            [
              ['health', 'Barre de vie'],
              ['armour', 'Barre d’armure'],
              ['compass', 'Boussole'],
            ] as const
          ).map(([key, label]) => (
            <Row key={key} label={label} stacked>
              <Segmented
                value={shape.hud[key]}
                options={HUD_MODES}
                onChange={(v) => patch({ hud: { ...shape.hud, [key]: v } })}
              />
            </Row>
          ))}
        </Section>

        <div className="px-1">
          <Button variant="filled" disabled>
            Exporter la resource
          </Button>
          <p className="text-footnote mt-2 text-label-3">
            Nécessite les fichiers du jeu — lot&nbsp;4 à venir.
          </p>
        </div>
      </aside>
    </div>
  )
}
