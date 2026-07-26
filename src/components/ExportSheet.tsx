import { AnimatePresence, motion } from 'motion/react'
import { useState } from 'react'
import type { MinimapShape } from '../lib/shape'
import {
  ddsInfo,
  exportResource,
  isTauri,
  pickDds,
  pickDirectory,
  revealInExplorer,
  type ExportReport,
} from '../lib/tauri'
import { Button, Row, Section, Switch, spring } from './ui'

/**
 * Dimensions par défaut des masques.
 *
 * Ce ne sont **pas** celles du jeu : elles ne peuvent pas l'être, `graphics.ytd`
 * n'étant pas distribuable. Le bouton « Lire un DDS vanilla » existe pour les
 * reprendre exactement, et c'est le chemin recommandé — un masque aux mauvaises
 * dimensions se réimporte mal dans le dictionnaire de textures.
 */
const DEFAULT_MASK: [number, number] = [256, 256]

type Size = [number, number]

export function ExportSheet({
  shape,
  open,
  onClose,
}: {
  shape: MinimapShape
  open: boolean
  onClose: () => void
}) {
  const [name, setName] = useState('ma-minimap')
  const [outDir, setOutDir] = useState<string | null>(null)
  const [enhanced, setEnhanced] = useState(false)
  const [maskSm, setMaskSm] = useState<Size>(DEFAULT_MASK)
  const [maskLg, setMaskLg] = useState<Size>(DEFAULT_MASK)
  const [busy, setBusy] = useState(false)
  const [report, setReport] = useState<ExportReport | null>(null)
  const [error, setError] = useState<string | null>(null)

  const readFromVanilla = async (which: 'sm' | 'lg') => {
    const path = await pickDds(
      which === 'sm' ? 'radarmasksm.dds vanilla' : 'radarmasklg.dds vanilla',
    )
    if (!path) return
    try {
      const info = await ddsInfo(path)
      const size: Size = [info.width, info.height]
      if (which === 'sm') setMaskSm(size)
      else setMaskLg(size)
    } catch (e) {
      setError(String(e))
    }
  }

  const run = async () => {
    if (!outDir) return
    setBusy(true)
    setError(null)
    try {
      setReport(
        await exportResource({ shape, name, outDir, enhanced, maskSm, maskLg }),
      )
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }

  const close = () => {
    setReport(null)
    setError(null)
    onClose()
  }

  return (
    <AnimatePresence>
      {open && (
        <>
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={close}
            className="fixed inset-0 z-40 bg-black/50 backdrop-blur-sm"
          />
          <motion.div
            initial={{ y: '100%' }}
            animate={{ y: 0 }}
            exit={{ y: '100%' }}
            transition={spring.sheet}
            className="material-thick fixed inset-x-0 bottom-0 z-50 max-h-[88%] overflow-y-auto rounded-t-ios-card border-t border-separator px-6 pb-8 pt-3"
          >
            {/* La poignée iOS : signale que la feuille est refermable. */}
            <div className="mx-auto mb-5 h-1 w-9 rounded-full bg-fill-1" />

            {report ? (
              <Result report={report} onClose={close} />
            ) : (
              <>
                <h2 className="text-title mb-5">Exporter la resource</h2>

                <Section>
                  <Row label="Nom" detail="Espaces et accents seront convertis" stacked>
                    <input
                      value={name}
                      onChange={(e) => setName(e.target.value)}
                      className="text-body w-full rounded-ios bg-fill-4 px-3 py-2 outline-none focus:ring-2 focus:ring-ios-blue"
                    />
                  </Row>
                  <Row label="Dossier de sortie" detail={outDir ?? 'Aucun dossier choisi'}>
                    <Button
                      variant="tinted"
                      onClick={async () => {
                        const d = await pickDirectory('Où écrire la resource')
                        if (d) setOutDir(d)
                      }}
                    >
                      Choisir
                    </Button>
                  </Row>
                  <Row label="GTA V Enhanced" detail="Décoché = Legacy (gen8)">
                    <Switch checked={enhanced} onChange={setEnhanced} />
                  </Row>
                </Section>

                <Section title="Dimensions des masques">
                  <SizeRow
                    label="radarmasksm"
                    detail="Minimap courante"
                    size={maskSm}
                    onChange={setMaskSm}
                    onRead={() => readFromVanilla('sm')}
                  />
                  <SizeRow
                    label="radarmasklg"
                    detail="Carte agrandie"
                    size={maskLg}
                    onChange={setMaskLg}
                    onRead={() => readFromVanilla('lg')}
                  />
                  <div className="text-footnote px-4 py-3 text-label-2">
                    Les valeurs par défaut sont arbitraires. Pour être exact, lisez-les
                    depuis les textures vanilla extraites de{' '}
                    <span className="font-mono">graphics.ytd</span>.
                  </div>
                </Section>

                {error && (
                  <p className="text-footnote mb-4 rounded-ios bg-ios-red/15 px-4 py-3 text-ios-red">
                    {error}
                  </p>
                )}

                <div className="flex items-center gap-3">
                  <Button
                    variant="filled"
                    disabled={!outDir || busy || !isTauri()}
                    onClick={run}
                  >
                    {busy ? 'Export…' : 'Exporter'}
                  </Button>
                  <Button onClick={close}>Annuler</Button>
                </div>
              </>
            )}
          </motion.div>
        </>
      )}
    </AnimatePresence>
  )
}

function SizeRow({
  label,
  detail,
  size,
  onChange,
  onRead,
}: {
  label: string
  detail: string
  size: Size
  onChange: (s: Size) => void
  onRead: () => void
}) {
  const field = (i: 0 | 1) => (
    <input
      type="number"
      min={1}
      max={4096}
      value={size[i]}
      onChange={(e) => {
        const v = parseInt(e.target.value, 10)
        const next: Size = [size[0], size[1]]
        // Un champ vidé donne NaN, qui produirait un masque de dimension nulle.
        next[i] = Number.isFinite(v) ? Math.min(4096, Math.max(1, v)) : 1
        onChange(next)
      }}
      className="text-body w-20 rounded-ios bg-fill-4 px-2 py-1.5 text-center font-mono tabular-nums outline-none focus:ring-2 focus:ring-ios-blue"
    />
  )

  return (
    <Row label={label} detail={detail} stacked>
      <div className="flex items-center gap-2">
        {field(0)}
        <span className="text-footnote text-label-3">×</span>
        {field(1)}
        <button
          onClick={onRead}
          className="text-footnote ml-auto rounded-ios bg-ios-blue/15 px-3 py-1.5 text-ios-blue"
        >
          Lire un DDS vanilla
        </button>
      </div>
    </Row>
  )
}

function Result({ report, onClose }: { report: ExportReport; onClose: () => void }) {
  return (
    <>
      <h2 className="text-title mb-1">Resource écrite</h2>
      <p className="text-footnote mb-5 break-all font-mono text-label-2">{report.root}</p>

      <Section title="Fichiers">
        <div className="px-4 py-3">
          {report.files.map((f) => (
            <div key={f} className="text-footnote font-mono text-label-2">
              {f}
            </div>
          ))}
        </div>
      </Section>

      {report.warnings.length > 0 && (
        <Section title="Il reste une étape">
          {report.warnings.map((w) => (
            <div key={w} className="text-footnote px-4 py-3 text-ios-orange">
              {w}
            </div>
          ))}
          <div className="text-footnote px-4 py-3 text-label-2">
            La marche à suivre est détaillée dans le{' '}
            <span className="font-mono">LISEZ-MOI.md</span> du dossier.
          </div>
        </Section>
      )}

      {report.limitations.length > 0 && (
        <Section title="Limites">
          {report.limitations.map((l) => (
            <div key={l} className="text-footnote px-4 py-3 text-label-2">
              {l}
            </div>
          ))}
        </Section>
      )}

      <div className="flex items-center gap-3">
        <Button variant="filled" onClick={() => revealInExplorer(report.root)}>
          Ouvrir le dossier
        </Button>
        <Button onClick={onClose}>Fermer</Button>
      </div>
    </>
  )
}
