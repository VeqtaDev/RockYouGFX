import { AnimatePresence, motion } from 'motion/react'
import { useEffect, useState } from 'react'
import { checkForUpdate, type UpdateInfo } from '../lib/update'
import { spring } from './ui'

/**
 * Bandeau de mise à jour.
 *
 * S'affiche uniquement si une version plus récente existe : hors ligne ou à
 * jour, le composant ne rend rien du tout.
 */
export function UpdateBanner() {
  const [update, setUpdate] = useState<UpdateInfo | null>(null)
  const [dismissed, setDismissed] = useState(false)

  useEffect(() => {
    // Annulé au démontage : en développement, le double montage du StrictMode
    // déclencherait autrement deux requêtes.
    const ctrl = new AbortController()
    checkForUpdate(ctrl.signal).then(setUpdate)
    return () => ctrl.abort()
  }, [])

  const visible = update !== null && !dismissed
  const href = update?.portableUrl ?? update?.setupUrl ?? update?.url

  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          initial={{ y: -60, opacity: 0 }}
          animate={{ y: 0, opacity: 1 }}
          exit={{ y: -60, opacity: 0 }}
          transition={spring.sheet}
          className="material absolute inset-x-6 top-6 z-50 flex items-center gap-3 rounded-ios-lg border border-separator px-4 py-3 shadow-2xl"
        >
          <span className="flex h-2 w-2 shrink-0 rounded-full bg-ios-blue" />
          <div className="min-w-0 flex-1">
            <div className="text-headline">Version {update.version} disponible</div>
            <div className="text-footnote text-label-2">
              Vous utilisez la {__APP_VERSION__}.
            </div>
          </div>
          <a
            href={href}
            target="_blank"
            rel="noreferrer"
            className="text-headline rounded-ios bg-ios-blue px-4 py-2 text-white"
          >
            Télécharger
          </a>
          <button
            onClick={() => setDismissed(true)}
            aria-label="Ignorer"
            className="text-headline px-2 text-label-2"
          >
            ✕
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
