import type { Metadata } from 'next'
import { DM_Mono, DM_Sans } from 'next/font/google'
import { Providers } from './providers'
import './globals.css'

const dmSans = DM_Sans({
  variable: '--font-sans',
  subsets: ['latin'],
})

const dmMono = DM_Mono({
  variable: '--font-mono',
  weight: ['400', '500'],
  subsets: ['latin'],
})

export const metadata: Metadata = {
  title: 'Axon',
  description: 'Neural RAG Pipeline',
}

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode
}>) {
  return (
    <html lang="en" className="dark">
      <body className={`${dmSans.variable} ${dmMono.variable} antialiased`}>
        <Providers>{children}</Providers>
      </body>
    </html>
  )
}
