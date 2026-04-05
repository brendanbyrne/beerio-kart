import { useCallback, useEffect, useState } from 'react'
import { apiFetch } from '../api/client'
import type { DrinkType, SimpleItem } from '../api/types'

/** Fetches and caches a list of simple game data items. */
function useSimpleList(endpoint: string) {
  const [items, setItems] = useState<SimpleItem[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function load() {
      try {
        const res = await apiFetch(endpoint)
        const data = await res.json()
        setItems(data)
      } catch {
        // Silently fail — items stay empty
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [endpoint])

  return { items, loading }
}

export function useCharacters() {
  return useSimpleList('/api/v1/characters')
}

export function useBodies() {
  return useSimpleList('/api/v1/bodies')
}

export function useWheels() {
  return useSimpleList('/api/v1/wheels')
}

export function useGliders() {
  return useSimpleList('/api/v1/gliders')
}

export function useDrinkTypes() {
  const [items, setItems] = useState<DrinkType[]>([])
  const [loading, setLoading] = useState(true)
  const [version, setVersion] = useState(0)

  const refresh = useCallback(() => {
    setVersion((v) => v + 1)
  }, [])

  useEffect(() => {
    async function load() {
      try {
        const res = await apiFetch('/api/v1/drink-types')
        const data = await res.json()
        setItems(data)
      } catch {
        // Silently fail
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [version])

  return { items, loading, refresh }
}
