/**
 * Vérification de mise à jour contre les releases GitHub.
 *
 * L'appel se fait depuis le frontend plutôt que côté Rust : cela évite
 * d'embarquer un client HTTP complet dans le binaire, dont la taille compte
 * directement puisque c'est l'exécutable portable qui est distribué.
 *
 * `connect-src` de la CSP doit autoriser api.github.com — voir tauri.conf.json.
 */

const REPO = 'VeqtaDev/RockYouGFX'

// On liste les releases au lieu d'interroger /releases/latest : cet endpoint
// **exclut les préversions** et renvoie 404 tant qu'aucune release stable n'a
// été publiée. Le projet publiant ses premières versions en préversion, la
// vérification n'aurait jamais rien détecté.
const API = `https://api.github.com/repos/${REPO}/releases?per_page=10`

/**
 * Injectée au build depuis la version du package.json (voir vite.config.ts).
 *
 * Le `typeof` garde le module importable hors bundler — tests unitaires,
 * scripts Node — où le remplacement n'a pas lieu. Vite substitue aussi le
 * jeton dans le `typeof`, donc le repli ne coûte rien au build.
 */
export const CURRENT_VERSION: string =
  typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : '0.0.0'

export interface UpdateInfo {
  version: string
  url: string
  notes: string
  /** Exécutable portable, absent si la release ne le publie pas. */
  portableUrl?: string
  /** Installeur NSIS. */
  setupUrl?: string
}

interface GhAsset {
  name: string
  browser_download_url: string
}

/**
 * Compare deux versions sémantiques.
 *
 * Renvoie un nombre positif si `a` est plus récente que `b`. Une comparaison
 * de chaînes serait fausse dès la version 10 ("10" < "9" lexicographiquement).
 */
export function compareVersions(a: string, b: string): number {
  const parse = (v: string) =>
    v
      .replace(/^v/, '')
      // Une préversion ("1.2.0-beta.1") n'est pas ordonnée ici : seule la
      // partie numérique est comparée, ce qui suffit à détecter une nouveauté.
      .split('-')[0]!
      .split('.')
      .map((n) => parseInt(n, 10) || 0)

  const pa = parse(a)
  const pb = parse(b)
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0)
    if (d !== 0) return d
  }
  return 0
}

/**
 * Renvoie la release si elle est plus récente que la version courante,
 * sinon `null`.
 *
 * Toute erreur réseau est avalée : une vérification de mise à jour ne doit
 * jamais empêcher l'application de fonctionner hors ligne.
 */
export async function checkForUpdate(signal?: AbortSignal): Promise<UpdateInfo | null> {
  try {
    const res = await fetch(API, {
      signal,
      headers: { Accept: 'application/vnd.github+json' },
    })
    if (!res.ok) return null

    const releases = (await res.json()) as {
      tag_name?: string
      html_url?: string
      body?: string
      draft?: boolean
      assets?: GhAsset[]
    }[]
    if (!Array.isArray(releases)) return null

    // Les brouillons ne sont pas publics : les proposer enverrait vers un 404.
    const data = releases.find((r) => !r.draft)
    if (!data) return null

    const tag = data.tag_name
    if (!tag) return null
    if (compareVersions(tag, CURRENT_VERSION) <= 0) return null

    const assets = data.assets ?? []
    const find = (suffix: string) =>
      assets.find((a) => a.name.toLowerCase().endsWith(suffix))?.browser_download_url

    return {
      version: tag.replace(/^v/, ''),
      url: data.html_url ?? `https://github.com/${REPO}/releases/latest`,
      notes: data.body ?? '',
      portableUrl: find('portable.exe'),
      setupUrl: find('setup.exe'),
    }
  } catch {
    return null
  }
}
