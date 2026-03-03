'use client'

import { useCallback, useEffect, useRef, useState } from 'react'

/** Shows a notice string for a fixed duration, then auto-clears it. */
export function useTimedNotice(durationMs = 1800) {
  const [notice, setNotice] = useState<string | null>(null)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const showNotice = useCallback(
    (message: string) => {
      if (timerRef.current) clearTimeout(timerRef.current)
      setNotice(message)
      timerRef.current = setTimeout(() => {
        setNotice(null)
        timerRef.current = null
      }, durationMs)
    },
    [durationMs],
  )

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current)
    }
  }, [])

  const clearNotice = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }
    setNotice(null)
  }, [])

  return { notice, showNotice, clearNotice } as const
}
