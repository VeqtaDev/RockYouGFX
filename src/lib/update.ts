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
  /** Nom du fichier portable, pour retrouver son empreinte dans SHA256SUMS. */
  portableName?: string
  /** Installeur NSIS. */
  setupUrl?: string
  /** Empreintes publiées avec la release. Absentes des releases antérieures à la v0.1.2. */
  checksumsUrl?: string
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
    const findAsset = (suffix: string) =>
      assets.find((a) => a.name.toLowerCase().endsWith(suffix))
    const portable = findAsset('portable.exe')

    return {
      version: tag.replace(/^v/, ''),
      url: data.html_url ?? `https://github.com/${REPO}/releases/latest`,
      notes: data.body ?? '',
      portableUrl: portable?.browser_download_url,
      portableName: portable?.name,
      setupUrl: findAsset('setup.exe')?.browser_download_url,
      checksumsUrl: assets.find((a) => a.name === 'SHA256SUMS.txt')
        ?.browser_download_url,
    }
  } catch {
    return null
  }
}

/**
 * Extrait l'empreinte d'un fichier d'un contenu au format `sha256sum`.
 *
 * Chaque ligne vaut `<empreinte>  <nom de fichier>`, avec deux espaces. Le nom
 * peut être préfixé de `*` en mode binaire, d'où le nettoyage.
 */
export function parseChecksums(text: string, fileName: string): string | null {
  for (const line of text.split('\n')) {
    const m = line.trim().match(/^([0-9a-fA-F]{64})\s+\*?(.+)$/)
    if (m && m[2]!.trim() === fileName) return m[1]!.toLowerCase()
  }
  return null
}

/**
 * Télécharge le portable et son empreinte.
 *
 * Renvoie l'octet brut et l'empreinte attendue ; c'est le backend qui vérifie
 * et remplace, la vérification devant précéder toute écriture sur le disque.
 */
export async function fetchPortableUpdate(
  info: UpdateInfo,
  onProgress?: (received: number, total: number) => void,
): Promise<{ bytes: Uint8Array; sha256: string }> {
  if (!info.portableUrl || !info.portableName) {
    throw new Error("cette release ne publie pas d'exécutable portable")
  }
  if (!info.checksumsUrl) {
    throw new Error(
      "cette release ne publie pas d'empreintes : mise à jour automatique impossible",
    )
  }

  const sumsRes = await fetch(info.checksumsUrl)
  if (!sumsRes.ok) throw new Error(`empreintes inaccessibles (${sumsRes.status})`)
  const sha256 = parseChecksums(await sumsRes.text(), info.portableName)
  if (!sha256) throw new Error(`aucune empreinte pour ${info.portableName}`)

  const res = await fetch(info.portableUrl)
  if (!res.ok) throw new Error(`téléchargement échoué (${res.status})`)

  const total = Number(res.headers.get('content-length') ?? 0)
  const reader = res.body?.getReader()
  if (!reader) {
    return { bytes: new Uint8Array(await res.arrayBuffer()), sha256 }
  }

  const chunks: Uint8Array[] = []
  let received = 0
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    chunks.push(value)
    received += value.length
    onProgress?.(received, total)
  }

  const bytes = new Uint8Array(received)
  let offset = 0
  for (const c of chunks) {
    bytes.set(c, offset)
    offset += c.length
  }
  return { bytes, sha256 }
}
