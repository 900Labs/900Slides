import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { mergeDeckSnapshot } from './deckSnapshot.js'
import type { DeckSnapshot, MediaMap } from './types'

type DeckSnapshotWire = Omit<DeckSnapshot, 'media'> & { media?: MediaMap }

function isDeckSnapshotWire(value: unknown): value is DeckSnapshotWire {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Partial<DeckSnapshotWire>
  return (
    typeof candidate.id === 'string' &&
    typeof candidate.mediaRevision === 'number' &&
    Array.isArray(candidate.slides)
  )
}

/**
 * Application IPC wrapper. Deck snapshots are detected structurally and have
 * revision-scoped media restored; every other command result passes through.
 */
export async function invokeApp<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const value = await tauriInvoke<T>(command, args)
  if (!isDeckSnapshotWire(value)) return value
  try {
    return mergeDeckSnapshot(value) as T
  } catch (error) {
    const full = mergeDeckSnapshot(await tauriInvoke<DeckSnapshotWire>('get_snapshot'))
    if (full.id === value.id && full.mediaRevision === value.mediaRevision) {
      return full as T
    }
    throw error
  }
}

/** Invokes a deck command and restores an omitted unchanged media payload. */
export async function invokeDeckSnapshot(
  command: string,
  args?: Record<string, unknown>,
): Promise<DeckSnapshot> {
  return invokeApp<DeckSnapshot>(command, args)
}
