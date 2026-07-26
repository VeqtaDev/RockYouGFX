import { AnimatePresence, motion } from 'motion/react'
import { useEffect, useState } from 'react'
import { checkForUpdate, fetchPortableUpdate, type UpdateInfo } from '../lib/update'
import { applyPortableUpdate, canSelfUpdate, isTauri } from '../lib/tauri'
import { spring } from './ui'

type State =
  | { kind: 'idle' }
  | { kind: 'downloading'; pct: number }
  | { kind: 'restarting' }
  | { kind: 'error'; message: string }

/**
 * Bandeau de mise à jour.
 *
 * S'affiche uniquement si une version plus récente existe : hors ligne ou à
 * jour, le composant ne rend rien du tout.
 *
 * Le remplacement automatique n'est proposé qu'à la version portable. La
 * version installée est gérée par son installeur, et son dossier n'est de
 * toute façon pas accessible en écriture sans élévation.
 */
export function UpdateBanner() {
  const [update, setUpdate] = useState<UpdateInfo | null>(null)
  const [selfUpdate, setSelfUpdate] = useState(false)
  const [dismissed, setDismissed] = useState(false)
  const [state, setState] = useState<State>({ kind: 'idle' })

  useEffect(() => {
    // Annulé au démontage : en développement, le double montage du StrictMode
    // déclencherait autrement deux requêtes.
    const ctrl = new AbortController()
    checkForUpdate(ctrl.signal).then(setUpdate)
    if (isTauri()) canSelfUpdate().then(setSelfUpdate).catch(() => setSelfUpdate(false))
    return () => ctrl.abort()
  }, [])

  const run = async () => {
    if (!update) return
    setState({ kind: 'downloading', pct: 0 })
    try {
      const { bytes, sha256 } = await fetchPortableUpdate(update, (got, total) => {
        // Sans en-tête content-length, on ne peut pas afficher de pourcentage.
        setState({ kind: 'downloading', pct: total > 0 ? got / total : 0 })
      })
      setState({ kind: 'restarting' })
      await applyPortableUpdate(bytes, sha256)
    } catch (e) {
      setState({ kind: 'error', message: String(e) })
    }
  }

  const visible = update !== null && !dismissed
  // Le remplacement exige les empreintes : les releases antérieures à la
  // v0.1.2 n'en publient pas, on retombe alors sur le téléchargement manuel.
  const auto = selfUpdate && !!update?.checksumsUrl && !!update?.portableUrl
  const href = update?.portableUrl ?? update?.setupUrl ?? update?.url

  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          initial={{ y: -60, opacity: 0 }}
          animate={{ y: 0, opacity: 1 }}
          exit={{ y: -60, opacity: 0 }}
          transition={spring.sheet}
          className="material absolute inset-x-6 top-6 z-50 overflow-hidden rounded-ios-lg border border-separator shadow-2xl"
        >
          <div className="flex items-center gap-3 px-4 py-3">
            <span className="flex h-2 w-2 shrink-0 rounded-full bg-ios-blue" />
            <div className="min-w-0 flex-1">
              <div className="text-headline">Version {update.version} disponible</div>
              <div className="text-footnote text-label-2">
                {state.kind === 'downloading'
                  ? `Téléchargement… ${Math.round(state.pct * 100)} %`
                  : state.kind === 'restarting'
                    ? 'Installation, l’application va redémarrer…'
                    : state.kind === 'error'
                      ? state.message
                      : `Vous utilisez la ${__APP_VERSION__}.`}
              </div>
            </div>

            {state.kind === 'idle' &&
              (auto ? (
                <button
                  onClick={run}
                  className="text-headline rounded-ios bg-ios-blue px-4 py-2 text-white"
                >
                  Mettre à jour
                </button>
              ) : (
                <a
                  href={href}
                  target="_blank"
                  rel="noreferrer"
                  className="text-headline rounded-ios bg-ios-blue px-4 py-2 text-white"
                >
                  Télécharger
                </a>
              ))}

            {state.kind === 'error' && (
              <a
                href={href}
                target="_blank"
                rel="noreferrer"
                className="text-headline rounded-ios bg-fill-3 px-4 py-2"
              >
                Télécharger
              </a>
            )}

            <button
              onClick={() => setDismissed(true)}
              aria-label="Ignorer"
              className="text-headline px-2 text-label-2"
            >
              ✕
            </button>
          </div>

          {state.kind === 'downloading' && (
            <div className="h-0.5 w-full bg-fill-4">
              <motion.div
                className="h-full bg-ios-blue"
                animate={{ width: `${state.pct * 100}%` }}
                transition={{ duration: 0.2 }}
              />
            </div>
          )}
        </motion.div>
      )}
    </AnimatePresence>
  )
}
