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
}

export interface ExportRequest {
  shape: MinimapShape
  name: string
  outDir: string
  enhanced: boolean
  maskSm: [number, number]
  maskLg: [number, number]
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

export function revealInExplorer(path: string): Promise<void> {
  return revealItemInDir(path)
}
