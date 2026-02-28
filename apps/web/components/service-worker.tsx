'use client'

import { useEffect } from 'react'

export function ServiceWorkerRegistration() {
  useEffect(() => {
    const isLikelyLocalhost =
      location.hostname === 'localhost' ||
      location.hostname === '127.0.0.1' ||
      location.hostname === '[::1]'

    if (!window.isSecureContext && !isLikelyLocalhost) {
      console.warn(
        '[PWA] Insecure context detected. Chrome on Android requires HTTPS for installability on non-localhost origins.',
        { origin: location.origin },
      )
      return
    }

    if (!('serviceWorker' in navigator)) {
      console.warn('[PWA] Service workers are not supported in this browser/context.', {
        origin: location.origin,
        isSecureContext: window.isSecureContext,
      })
      return
    }

    navigator.serviceWorker
      .register('/sw.js', { scope: '/' })
      .then((registration) => {
        console.info('[PWA] Service worker registered.', { scope: registration.scope })
        registration.onupdatefound = () => {
          const installingWorker = registration.installing
          if (!installingWorker) return
          installingWorker.onstatechange = () => {
            if (installingWorker.state === 'installed' && navigator.serviceWorker.controller) {
              console.info('[PWA] New service worker available. Reloading...')
              window.location.reload()
            }
          }
        }
      })
      .catch((error) => {
        console.error('[PWA] Service worker registration failed.', error)
      })
  }, [])

  return null
}
