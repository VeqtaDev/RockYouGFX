/**
 * Pont vers le backend Tauri.
 *
 * L'éditeur tourne aussi dans un navigateur (`pnpm dev`), où aucune de ces
 * commandes n'existe. Tout passe donc par `isTauri()` : l'interface désactive
 * l'export au lieu de lever une erreur opaque au clic.
 */
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import type { MinimapShape } from './shape'

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export interface DdsInfo {
  width: number
  height: number
  fourCC: string | null
  bitCount: number
}

export interface ExportReport {
  root: string
  files: string[]
  warnings: string[]
  limitations: string[]
  /** Nom réellement utilisé : le backend assainit espaces et accents. */
  resourceName: string
  /** Textures remplacées dans le graphics.ytd. Vide si aucun n'a été fourni. */
  patchedTextures: string[]
}

export interface ExportRequest {
  shape: MinimapShape
  name: string
  outDir: string
  enhanced: boolean
  maskSm: [number, number]
  maskLg: [number, number]
  /** graphics.ytd vanilla à patcher. Sans lui, l'injection reste manuelle. */
  graphicsYtdPath?: string
}

export function exportResource(req: ExportRequest): Promise<ExportReport> {
  return invoke<ExportReport>('export_resource', { req })
}

/** Dimensions et format d'un DDS, pour reprendre celles de la texture vanilla. */
export function ddsInfo(path: string): Promise<DdsInfo> {
  return invoke<DdsInfo>('dds_info', { path })
}

export async function pickDirectory(title: string): Promise<string | null> {
  const res = await open({ directory: true, multiple: false, title })
  return typeof res === 'string' ? res : null
}

export async function pickDds(title: string): Promise<string | null> {
  const res = await open({
    multiple: false,
    title,
    filters: [{ name: 'Texture DDS', extensions: ['dds'] }],
  })
  return typeof res === 'string' ? res : null
}

export async function pickYtd(title: string): Promise<string | null> {
  const res = await open({
    multiple: false,
    title,
    filters: [{ name: 'Dictionnaire de textures', extensions: ['ytd'] }],
  })
  return typeof res === 'string' ? res : null
}

export function revealInExplorer(path: string): Promise<void> {
  return revealItemInDir(path)
}

/** Faux pour une installation NSIS, qui se met à jour par son installeur. */
export function canSelfUpdate(): Promise<boolean> {
  return invoke<boolean>('can_self_update')
}

/**
 * Remplace le binaire portable puis relance.
 *
 * L'appel ne rend jamais la main en cas de succès : le backend termine le
 * processus courant après avoir lancé le nouveau.
 */
export function applyPortableUpdate(bytes: Uint8Array, sha256: string): Promise<void> {
  // Transmis en base64, pas en tableau d'octets : l'IPC de Tauri sérialise en
  // JSON, et 3 Mo deviendraient un tableau de trois millions de nombres.
  return invoke('apply_portable_update', { payloadB64: toBase64(bytes), sha256 })
}

function toBase64(bytes: Uint8Array): string {
  // Par tranches : `String.fromCharCode(...)` sur plusieurs millions d'octets
  // dépasse la taille maximale de la pile d'arguments.
  const CHUNK = 0x8000
  let binary = ''
  for (let i = 0; i < bytes.length; i += CHUNK) {
    binary += String.fromCharCode(...bytes.subarray(i, i + CHUNK))
  }
  return btoa(binary)
}
